//! **OAuth 2.0 / OIDC federated login** (issue #63) — "Log in with Google /
//! Microsoft / Apple". Sandbox-first and credential-gated, mirroring
//! [`crate::providers`]: unless `LIVE_PROVIDERS` names the provider (and its
//! client credentials are in the secrets vault), a hermetic **sandbox
//! provider** runs — no network, deterministic — so CI and demos work offline.
//!
//! The authorization-code flow (with PKCE) is carried across the browser
//! redirect by a **signed state token** (JWT, our `jwt_secret`); the sandbox
//! provider hands back a **signed code** encoding the simulated account. The
//! HTTP surface + provisioning live in [`crate::routes::auth::oauth`].

use crate::config::Config;
use crate::error::{ApiError, ApiResult};
use base64::Engine;
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The providers we support (the OIDC subset that covers the DoD).
pub const PROVIDERS: &[&str] = &["google", "microsoft", "apple"];

pub fn is_valid_provider(p: &str) -> bool {
    PROVIDERS.contains(&p)
}

/// Real credentials vs the sandbox — same `LIVE_PROVIDERS` gate as every other
/// integration.
pub fn is_live(provider: &str) -> bool {
    crate::providers::is_live(provider)
}

struct Endpoints {
    authorize: &'static str,
    token: &'static str,
    scope: &'static str,
}

fn endpoints(provider: &str) -> Option<Endpoints> {
    match provider {
        "google" => Some(Endpoints {
            authorize: "https://accounts.google.com/o/oauth2/v2/auth",
            token: "https://oauth2.googleapis.com/token",
            scope: "openid email profile",
        }),
        "microsoft" => Some(Endpoints {
            authorize: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            token: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            scope: "openid email profile",
        }),
        "apple" => Some(Endpoints {
            authorize: "https://appleid.apple.com/auth/authorize",
            token: "https://appleid.apple.com/auth/token",
            scope: "openid email name",
        }),
        _ => None,
    }
}

/// The frontend base (where the browser lands after consent).
pub fn public_app_url() -> String {
    std::env::var("PUBLIC_APP_URL")
        .map(|v| v.trim_end_matches('/').to_string())
        .unwrap_or_else(|_| "http://localhost:3000".into())
}

/// The API base (where the sandbox provider's authorize endpoint lives).
pub fn public_api_url() -> String {
    std::env::var("PUBLIC_API_URL")
        .map(|v| v.trim_end_matches('/').to_string())
        .unwrap_or_else(|_| "http://localhost:8000".into())
}

fn redirect_uri() -> String {
    format!("{}/auth/callback", public_app_url())
}

// ---------------------------------------------------------------------------
// Signed state (carries the flow across the redirect) + sandbox code
// ---------------------------------------------------------------------------

const STATE_TYP: &str = "oauth_state";
const CODE_TYP: &str = "oauth_sandbox_code";
const FLOW_TTL_SECS: i64 = 600;

#[derive(Serialize, Deserialize)]
struct StateClaims {
    typ: String,
    provider: String,
    intent: String,
    tenant_id: Option<Uuid>,
    link_user_id: Option<Uuid>,
    pkce_verifier: String,
    nonce: String,
    iat: i64,
    exp: i64,
}

#[derive(Serialize, Deserialize)]
struct SandboxCodeClaims {
    typ: String,
    provider: String,
    sub: String,
    email: String,
    name: String,
    iat: i64,
    exp: i64,
}

fn hs256_key(cfg: &Config) -> (EncodingKey, DecodingKey) {
    (
        EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )
}

fn sign_state(cfg: &Config, claims: &StateClaims) -> anyhow::Result<String> {
    Ok(encode(&Header::default(), claims, &hs256_key(cfg).0)?)
}

fn verify_state(cfg: &Config, token: &str) -> Option<StateClaims> {
    let data =
        decode::<StateClaims>(token, &hs256_key(cfg).1, &Validation::new(Algorithm::HS256)).ok()?;
    (data.claims.typ == STATE_TYP).then_some(data.claims)
}

fn sign_sandbox_code(cfg: &Config, c: &SandboxCodeClaims) -> anyhow::Result<String> {
    Ok(encode(&Header::default(), c, &hs256_key(cfg).0)?)
}

fn verify_sandbox_code(cfg: &Config, token: &str) -> Option<SandboxCodeClaims> {
    let data =
        decode::<SandboxCodeClaims>(token, &hs256_key(cfg).1, &Validation::new(Algorithm::HS256))
            .ok()?;
    (data.claims.typ == CODE_TYP).then_some(data.claims)
}

// ---------------------------------------------------------------------------
// Flow
// ---------------------------------------------------------------------------

/// The external account resolved from a completed flow.
pub struct ExternalIdentity {
    pub provider: String,
    pub subject: String,
    pub email: String,
    pub name: Option<String>,
}

/// The parts of the (validated) state a route needs to complete a login/link.
pub struct FlowState {
    pub intent: String,
    pub tenant_id: Option<Uuid>,
    pub link_user_id: Option<Uuid>,
}

pub struct StartResult {
    pub authorize_url: String,
    pub sandbox: bool,
}

/// Percent-encode a query value (RFC 3986 unreserved set kept).
fn qenc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn pkce_challenge(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Begin a flow: build the provider authorize URL and a signed state token.
pub async fn start<C: ConnectionTrait>(
    cfg: &Config,
    db: &C,
    provider: &str,
    intent: &str,
    tenant_id: Option<Uuid>,
    link_user_id: Option<Uuid>,
) -> ApiResult<StartResult> {
    let verifier = crate::auth::random_secret(32);
    let now = Utc::now().timestamp();
    let state_claims = StateClaims {
        typ: STATE_TYP.into(),
        provider: provider.into(),
        intent: intent.into(),
        tenant_id,
        link_user_id,
        pkce_verifier: verifier.clone(),
        nonce: crate::auth::random_secret(16),
        iat: now,
        exp: now + FLOW_TTL_SECS,
    };
    let state = sign_state(cfg, &state_claims).map_err(ApiError::Internal)?;

    if is_live(provider) {
        let ep = endpoints(provider)
            .ok_or_else(|| ApiError::BadRequest(format!("unknown provider '{provider}'")))?;
        let client_id = crate::secrets::reveal(db, None, &format!("oauth.{provider}.client_id"))
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| {
                ApiError::BadRequest(format!("{provider} OAuth client_id is not configured"))
            })?;
        let challenge = pkce_challenge(&verifier);
        let authorize_url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}\
             &code_challenge={}&code_challenge_method=S256",
            ep.authorize,
            qenc(&client_id),
            qenc(&redirect_uri()),
            qenc(ep.scope),
            qenc(&state),
            qenc(&challenge),
        );
        Ok(StartResult {
            authorize_url,
            sandbox: false,
        })
    } else {
        // The sandbox "provider" is our own endpoint; the frontend opens it, the
        // user picks the simulated email, and it redirects back to /auth/callback.
        let authorize_url = format!(
            "{}/auth/oauth/{}/sandbox?state={}",
            public_api_url(),
            provider,
            qenc(&state),
        );
        Ok(StartResult {
            authorize_url,
            sandbox: true,
        })
    }
}

/// Sandbox provider: validate the state, mint a signed code for the simulated
/// account, and return the redirect back to the app callback.
pub fn sandbox_redirect(
    cfg: &Config,
    provider: &str,
    state: &str,
    email: Option<&str>,
) -> ApiResult<String> {
    let st = verify_state(cfg, state)
        .ok_or_else(|| ApiError::BadRequest("invalid or expired state".into()))?;
    if st.provider != provider {
        return Err(ApiError::BadRequest("state/provider mismatch".into()));
    }
    let email = email
        .map(|e| e.trim().to_lowercase())
        .filter(|e| e.contains('@'))
        .unwrap_or_else(|| format!("sandbox.user@{provider}.example"));
    let name = derive_name(&email);
    let sub = sandbox_subject(provider, &email);
    let now = Utc::now().timestamp();
    let code = sign_sandbox_code(
        cfg,
        &SandboxCodeClaims {
            typ: CODE_TYP.into(),
            provider: provider.into(),
            sub,
            email,
            name,
            iat: now,
            exp: now + FLOW_TTL_SECS,
        },
    )
    .map_err(ApiError::Internal)?;
    Ok(format!(
        "{}/auth/callback?provider={}&code={}&state={}",
        public_app_url(),
        provider,
        qenc(&code),
        qenc(state),
    ))
}

/// A stable, deterministic sandbox subject for an email (so re-logins map to the
/// same account).
fn sandbox_subject(provider: &str, email: &str) -> String {
    let hex = crate::auth::hash_secret(&format!("{provider}:{email}"));
    format!("{provider}-sandbox-{}", &hex[..16])
}

fn derive_name(email: &str) -> String {
    let local = email.split('@').next().unwrap_or(email);
    local
        .split(['.', '_', '-'])
        .filter(|s| !s.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Exchange a callback (`code` + `state`) for the external identity and the
/// flow's state. Sandbox decodes the signed code; live exchanges at the token
/// endpoint.
pub async fn exchange<C: ConnectionTrait>(
    cfg: &Config,
    db: &C,
    provider: &str,
    code: &str,
    state: &str,
) -> ApiResult<(ExternalIdentity, FlowState)> {
    let st = verify_state(cfg, state)
        .ok_or_else(|| ApiError::BadRequest("invalid or expired state".into()))?;
    if st.provider != provider {
        return Err(ApiError::BadRequest("state/provider mismatch".into()));
    }
    let flow = FlowState {
        intent: st.intent.clone(),
        tenant_id: st.tenant_id,
        link_user_id: st.link_user_id,
    };

    let identity = if is_live(provider) {
        exchange_live(db, provider, code, &st.pkce_verifier).await?
    } else {
        let c = verify_sandbox_code(cfg, code)
            .ok_or_else(|| ApiError::BadRequest("invalid sandbox code".into()))?;
        if c.provider != provider {
            return Err(ApiError::BadRequest(
                "sandbox code/provider mismatch".into(),
            ));
        }
        ExternalIdentity {
            provider: provider.into(),
            subject: c.sub,
            email: c.email.to_lowercase(),
            name: Some(c.name),
        }
    };
    Ok((identity, flow))
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    access_token: Option<String>,
}

#[derive(Deserialize)]
struct IdTokenClaims {
    sub: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// Live authorization-code exchange: POST to the token endpoint, then read the
/// identity from the returned `id_token`. The id_token's signature is not
/// re-verified because it arrived **directly** from the provider's token
/// endpoint over TLS (not via the browser) — the trust boundary of the
/// authorization-code flow. Never exercised in CI (sandbox is the default).
async fn exchange_live<C: ConnectionTrait>(
    db: &C,
    provider: &str,
    code: &str,
    verifier: &str,
) -> ApiResult<ExternalIdentity> {
    let ep = endpoints(provider)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown provider '{provider}'")))?;
    let creds = |k: &str| format!("oauth.{provider}.{k}");
    let client_id = crate::secrets::reveal(db, None, &creds("client_id"))
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::BadRequest("OAuth client_id not configured".into()))?;
    let client_secret = crate::secrets::reveal(db, None, &creds("client_secret"))
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::BadRequest("OAuth client_secret not configured".into()))?;

    let redirect = redirect_uri();
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect.as_str()),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("code_verifier", verifier),
    ];
    let http = reqwest::Client::new();
    let resp: TokenResponse = http
        .post(ep.token)
        .form(&form)
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .error_for_status()
        .map_err(|e| ApiError::Internal(e.into()))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    let id_token = resp
        .id_token
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("provider returned no id_token")))?;
    let claims = decode_id_token(&id_token)
        .ok_or_else(|| ApiError::Internal(anyhow::anyhow!("could not parse id_token")))?;
    let _ = resp.access_token; // userinfo fallback not needed with an id_token
    let email = claims
        .email
        .map(|e| e.trim().to_lowercase())
        .filter(|e| e.contains('@'))
        .ok_or_else(|| ApiError::BadRequest("provider did not return an email".into()))?;
    Ok(ExternalIdentity {
        provider: provider.into(),
        subject: claims.sub,
        email,
        name: claims.name,
    })
}

/// Read an OIDC id_token's claims without re-verifying its signature (see
/// [`exchange_live`] for why that's sound here).
fn decode_id_token(token: &str) -> Option<IdTokenClaims> {
    let mut v = Validation::new(Algorithm::RS256);
    v.algorithms = vec![Algorithm::RS256, Algorithm::ES256];
    v.insecure_disable_signature_validation();
    v.validate_exp = false;
    v.validate_aud = false;
    decode::<IdTokenClaims>(token, &DecodingKey::from_secret(b"unused"), &v)
        .ok()
        .map(|d| d.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> Config {
        Config {
            database_url: String::new(),
            jwt_secret: "oauth-test-secret-0123456789abcdef".into(),
            pii_key: vec![3u8; 32],
            secrets_key: vec![4u8; 32],
            access_ttl_secs: 900,
            refresh_ttl_secs: 1000,
            auto_migrate: false,
        }
    }

    #[test]
    fn valid_providers() {
        assert!(is_valid_provider("google"));
        assert!(!is_valid_provider("myspace"));
    }

    #[test]
    fn state_token_roundtrips_and_is_typed() {
        let cfg = cfg();
        let uid = Uuid::new_v4();
        let claims = StateClaims {
            typ: STATE_TYP.into(),
            provider: "google".into(),
            intent: "link".into(),
            tenant_id: None,
            link_user_id: Some(uid),
            pkce_verifier: "v".into(),
            nonce: "n".into(),
            iat: 0,
            exp: Utc::now().timestamp() + 100,
        };
        let token = sign_state(&cfg, &claims).unwrap();
        let back = verify_state(&cfg, &token).unwrap();
        assert_eq!(back.provider, "google");
        assert_eq!(back.link_user_id, Some(uid));
        assert!(verify_state(&cfg, "garbage").is_none());
        // A sandbox code must not validate as a state token (distinct `typ`).
        let code = sign_sandbox_code(
            &cfg,
            &SandboxCodeClaims {
                typ: CODE_TYP.into(),
                provider: "google".into(),
                sub: "s".into(),
                email: "e@x.com".into(),
                name: "E".into(),
                iat: 0,
                exp: Utc::now().timestamp() + 100,
            },
        )
        .unwrap();
        assert!(verify_state(&cfg, &code).is_none());
    }

    #[test]
    fn sandbox_subject_is_deterministic() {
        let a = sandbox_subject("google", "jo@x.com");
        let b = sandbox_subject("google", "jo@x.com");
        let c = sandbox_subject("google", "different@x.com");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("google-sandbox-"));
    }

    #[test]
    fn derive_name_titlecases_local_part() {
        assert_eq!(derive_name("jordan.mills@northwind.com"), "Jordan Mills");
        assert_eq!(derive_name("dana@x.com"), "Dana");
    }

    #[test]
    fn pkce_challenge_is_url_safe_no_pad() {
        let ch = pkce_challenge("verifier");
        assert!(!ch.contains('='));
        assert!(!ch.contains('+'));
        assert!(!ch.contains('/'));
    }
}
