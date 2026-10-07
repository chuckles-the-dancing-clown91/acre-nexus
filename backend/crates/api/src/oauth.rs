//! **OAuth 2.0 / OIDC federated login** (issue #63) — "Log in with Google /
//! Microsoft / Apple". Credential-gated, mirroring [`crate::providers`]: a
//! provider is **live** when `LIVE_PROVIDERS` names it (and its client
//! credentials are in the secrets vault). Otherwise a hermetic **sandbox
//! provider** — no network, deterministic, and it accepts *any* email — is
//! available, but **only** on a deployment that explicitly declares itself
//! non-production (`APP_ENV=development|dev|local|test|testing|ci`, see
//! [`Config::sandbox_auth`]). On production (or with `APP_ENV` unset) a provider
//! that isn't live is simply unavailable: the start, sandbox and callback steps
//! all refuse it, and [`available`] leaves it out so the login page hides it.
//!
//! The live path verifies the provider's ID token properly: signature against
//! the provider's published JWKS, `iss`, `aud` (= our client id), `exp`/`iat`,
//! the `nonce` we sent, and that the email is verified ([`verify_id_token`]).
//!
//! The authorization-code flow (with PKCE) is carried across the browser
//! redirect by a **signed state token** (JWT, our `jwt_secret`); the sandbox
//! provider hands back a **signed code** encoding the simulated account. The
//! HTTP surface + provisioning live in [`crate::routes::auth::oauth`].

use crate::config::Config;
use crate::error::{ApiError, ApiResult};
use base64::Engine;
use chrono::Utc;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{
    decode, decode_header, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation,
};
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
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

/// How a provider can be used on this deployment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Real OAuth against the provider (`LIVE_PROVIDERS` names it).
    Live,
    /// The simulated provider — non-production deployments only.
    Sandbox,
}

/// Pure decision: live wins; otherwise the sandbox, but only when the
/// deployment allows it; otherwise the provider is unavailable (`None`).
pub fn mode_for(live: bool, sandbox_allowed: bool) -> Option<Mode> {
    if live {
        Some(Mode::Live)
    } else if sandbox_allowed {
        Some(Mode::Sandbox)
    } else {
        None
    }
}

/// The mode `provider` runs in on this deployment (`None`: not available).
pub fn mode(cfg: &Config, provider: &str) -> Option<Mode> {
    if !is_valid_provider(provider) {
        return None;
    }
    mode_for(is_live(provider), cfg.sandbox_auth)
}

fn unavailable(provider: &str) -> ApiError {
    ApiError::Forbidden(format!("signing in with {provider} is not enabled here"))
}

/// The providers a sign-in page should offer: every live provider whose client
/// credentials are configured, plus (non-production only) the sandbox ones.
pub async fn available<C: ConnectionTrait>(cfg: &Config, db: &C) -> Vec<(&'static str, Mode)> {
    let mut out = Vec::new();
    for &p in PROVIDERS {
        match mode(cfg, p) {
            Some(Mode::Live) => {
                if has_secret(db, p, "client_id").await && has_secret(db, p, "client_secret").await
                {
                    out.push((p, Mode::Live));
                }
            }
            Some(Mode::Sandbox) => out.push((p, Mode::Sandbox)),
            None => {}
        }
    }
    out
}

async fn has_secret<C: ConnectionTrait>(db: &C, provider: &str, key: &str) -> bool {
    matches!(
        crate::secrets::reveal(db, None, &format!("oauth.{provider}.{key}")).await,
        Ok(Some(v)) if !v.trim().is_empty()
    )
}

struct Endpoints {
    authorize: &'static str,
    token: &'static str,
    scope: &'static str,
    /// The provider's published signing keys (JWKS).
    jwks: &'static str,
}

fn endpoints(provider: &str) -> Option<Endpoints> {
    match provider {
        "google" => Some(Endpoints {
            authorize: "https://accounts.google.com/o/oauth2/v2/auth",
            token: "https://oauth2.googleapis.com/token",
            scope: "openid email profile",
            jwks: "https://www.googleapis.com/oauth2/v3/certs",
        }),
        "microsoft" => Some(Endpoints {
            authorize: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize",
            token: "https://login.microsoftonline.com/common/oauth2/v2.0/token",
            scope: "openid email profile",
            jwks: "https://login.microsoftonline.com/common/discovery/v2.0/keys",
        }),
        "apple" => Some(Endpoints {
            authorize: "https://appleid.apple.com/auth/authorize",
            token: "https://appleid.apple.com/auth/token",
            scope: "openid email name",
            jwks: "https://appleid.apple.com/auth/keys",
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
    /// The provider vouches that the account owns `email` (always true for the
    /// sandbox; the live path refuses a token without it).
    pub email_verified: bool,
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
    let mode = mode(cfg, provider).ok_or_else(|| unavailable(provider))?;
    let verifier = crate::auth::random_secret(32);
    let nonce = crate::auth::random_secret(16);
    let now = Utc::now().timestamp();
    let state_claims = StateClaims {
        typ: STATE_TYP.into(),
        provider: provider.into(),
        intent: intent.into(),
        tenant_id,
        link_user_id,
        pkce_verifier: verifier.clone(),
        nonce: nonce.clone(),
        iat: now,
        exp: now + FLOW_TTL_SECS,
    };
    let state = sign_state(cfg, &state_claims).map_err(ApiError::Internal)?;

    if mode == Mode::Live {
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
             &nonce={}&code_challenge={}&code_challenge_method=S256",
            ep.authorize,
            qenc(&client_id),
            qenc(&redirect_uri()),
            qenc(ep.scope),
            qenc(&state),
            qenc(&nonce),
            qenc(&challenge),
        );
        Ok(StartResult {
            authorize_url,
            sandbox: false,
        })
    } else {
        // Non-production only (`mode` returned Sandbox): the sandbox "provider"
        // is our own endpoint; the frontend opens it, the user picks the
        // simulated email, and it redirects back to /auth/callback.
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
/// account, and return the redirect back to the app callback. Refused (404)
/// unless the deployment is explicitly non-production and the provider isn't
/// live — it signs in as whatever email it is handed.
pub fn sandbox_redirect(
    cfg: &Config,
    provider: &str,
    state: &str,
    email: Option<&str>,
) -> ApiResult<String> {
    if mode(cfg, provider) != Some(Mode::Sandbox) {
        return Err(ApiError::NotFound("not found".into()));
    }
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

    let identity = match mode(cfg, provider) {
        Some(Mode::Live) => exchange_live(db, provider, code, &st.pkce_verifier, &st.nonce).await?,
        Some(Mode::Sandbox) => sandbox_identity(cfg, provider, code)?,
        None => return Err(unavailable(provider)),
    };
    Ok((identity, flow))
}

/// Decode a sandbox code into the simulated identity (non-production only —
/// [`exchange`] never calls this unless [`mode`] says Sandbox).
fn sandbox_identity(cfg: &Config, provider: &str, code: &str) -> ApiResult<ExternalIdentity> {
    let c = verify_sandbox_code(cfg, code)
        .ok_or_else(|| ApiError::BadRequest("invalid sandbox code".into()))?;
    if c.provider != provider {
        return Err(ApiError::BadRequest(
            "sandbox code/provider mismatch".into(),
        ));
    }
    Ok(ExternalIdentity {
        provider: provider.into(),
        subject: c.sub,
        email: c.email.to_lowercase(),
        name: Some(c.name),
        email_verified: true,
    })
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    id_token: Option<String>,
}

/// Live authorization-code exchange: POST to the token endpoint, then **verify**
/// the returned `id_token` (signature against the provider's JWKS, issuer,
/// audience, expiry/issue time, nonce, verified email) before trusting any of
/// its claims. Never exercised in CI (no network); the verification itself is
/// unit-tested in [`verify_id_token`]'s tests.
async fn exchange_live<C: ConnectionTrait>(
    db: &C,
    provider: &str,
    code: &str,
    verifier: &str,
    nonce: &str,
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
    let http = http_client()?;
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
    let expect = Expected {
        provider,
        client_id: &client_id,
        nonce,
        now: Utc::now().timestamp(),
    };
    // A key we haven't seen may mean the provider rotated: refetch once.
    let mut jwks = provider_jwks(&http, provider, ep.jwks, false).await?;
    let mut result = verify_id_token(&id_token, &jwks, &expect);
    if matches!(result, Err(IdTokenError::UnknownKey)) {
        jwks = provider_jwks(&http, provider, ep.jwks, true).await?;
        result = verify_id_token(&id_token, &jwks, &expect);
    }
    let claims = result.map_err(|e| {
        tracing::warn!(provider, error = ?e, "rejected a provider id_token");
        e.into_api_error(provider)
    })?;
    Ok(ExternalIdentity {
        provider: provider.into(),
        subject: claims.sub,
        email: claims.email,
        name: claims.name,
        email_verified: true,
    })
}

fn http_client() -> ApiResult<reqwest::Client> {
    crate::providers::client::build_http_client()
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("{e:?}")))
}

// ---------------------------------------------------------------------------
// ID-token verification
// ---------------------------------------------------------------------------

/// Clock skew tolerated on `exp` / `iat`.
const ID_TOKEN_LEEWAY_SECS: i64 = 60;
/// How old an ID token may be when we receive it (it comes straight from the
/// token endpoint, so it is minted moments before).
const ID_TOKEN_MAX_AGE_SECS: i64 = FLOW_TTL_SECS;
/// How long a fetched JWKS is reused before it is refetched.
const JWKS_TTL: std::time::Duration = std::time::Duration::from_secs(3600);

/// What a provider's ID token must match.
pub(crate) struct Expected<'a> {
    pub provider: &'a str,
    /// Our OAuth client id — the token's `aud`.
    pub client_id: &'a str,
    /// The nonce we put in the authorize request (carried in the signed state).
    pub nonce: &'a str,
    /// "Now", unix seconds (injected for tests).
    pub now: i64,
}

/// The verified claims we use.
#[derive(Debug)]
pub(crate) struct VerifiedClaims {
    pub sub: String,
    pub email: String,
    pub name: Option<String>,
}

/// Why an ID token was refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IdTokenError {
    /// Not a JWT / undecodable header.
    Malformed,
    /// `alg` is not an asymmetric algorithm we accept (`none`, `HS256`, …).
    BadAlgorithm,
    /// No `kid`, or no key with that `kid` in the provider's JWKS.
    UnknownKey,
    /// Signature, `aud`, `exp`, or a required claim failed validation.
    Invalid(String),
    /// `iss` is not the provider's issuer.
    BadIssuer,
    /// `iat` is in the future or too old.
    BadIssuedAt,
    /// `nonce` missing or not the one we sent.
    BadNonce,
    /// No email, or the provider does not vouch that it is verified.
    EmailNotVerified,
}

impl IdTokenError {
    fn into_api_error(self, provider: &str) -> ApiError {
        match self {
            IdTokenError::EmailNotVerified => ApiError::Forbidden(format!(
                "your {provider} account's email address is not verified"
            )),
            _ => ApiError::Unauthorized,
        }
    }
}

#[derive(Deserialize)]
struct IdTokenClaims {
    sub: String,
    iss: String,
    iat: i64,
    #[serde(default)]
    nonce: Option<String>,
    #[serde(default)]
    email: Option<String>,
    /// Google: a bool. Apple: a bool or the string "true".
    #[serde(default)]
    email_verified: Option<serde_json::Value>,
    /// Microsoft's optional "email domain owner verified" claim (Entra does not
    /// send `email_verified`).
    #[serde(default)]
    xms_edov: Option<serde_json::Value>,
    /// Microsoft: the directory (tenant) id, embedded in its issuer.
    #[serde(default)]
    tid: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

fn truthy(v: &Option<serde_json::Value>) -> bool {
    match v {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::String(s)) => s.eq_ignore_ascii_case("true"),
        _ => false,
    }
}

/// Whether `iss` is `provider`'s issuer. Microsoft's is per directory
/// (`https://login.microsoftonline.com/{tid}/v2.0`), so it must match the
/// token's own `tid`.
fn issuer_ok(provider: &str, iss: &str, tid: Option<&str>) -> bool {
    match provider {
        "google" => iss == "https://accounts.google.com" || iss == "accounts.google.com",
        "apple" => iss == "https://appleid.apple.com",
        "microsoft" => tid.is_some_and(|t| {
            !t.is_empty() && iss == format!("https://login.microsoftonline.com/{t}/v2.0")
        }),
        _ => false,
    }
}

/// Verify a provider ID token against its JWKS and our expectations. Pure (no
/// I/O) so every refusal is unit-testable.
pub(crate) fn verify_id_token(
    token: &str,
    jwks: &JwkSet,
    expect: &Expected<'_>,
) -> Result<VerifiedClaims, IdTokenError> {
    let header = decode_header(token).map_err(|_| IdTokenError::Malformed)?;
    if !matches!(header.alg, Algorithm::RS256 | Algorithm::ES256) {
        return Err(IdTokenError::BadAlgorithm);
    }
    let kid = header.kid.as_deref().ok_or(IdTokenError::UnknownKey)?;
    let jwk = jwks.find(kid).ok_or(IdTokenError::UnknownKey)?;
    // A key published for one algorithm must not verify another.
    if let Some(ka) = &jwk.common.key_algorithm {
        if ka.to_string() != format!("{:?}", header.alg) {
            return Err(IdTokenError::BadAlgorithm);
        }
    }
    let key = DecodingKey::from_jwk(jwk).map_err(|_| IdTokenError::UnknownKey)?;

    let mut v = Validation::new(header.alg);
    v.algorithms = vec![header.alg];
    v.leeway = ID_TOKEN_LEEWAY_SECS as u64;
    v.validate_exp = true;
    v.set_audience(&[expect.client_id]);
    v.set_required_spec_claims(&["exp", "iat", "iss", "aud", "sub"]);
    // `iss` is checked below (Microsoft's depends on the token's `tid`).
    v.iss = None;
    let claims = decode::<IdTokenClaims>(token, &key, &v)
        .map_err(|e| IdTokenError::Invalid(format!("{:?}", e.kind())))?
        .claims;

    if !issuer_ok(expect.provider, &claims.iss, claims.tid.as_deref()) {
        return Err(IdTokenError::BadIssuer);
    }
    if claims.iat > expect.now + ID_TOKEN_LEEWAY_SECS
        || claims.iat < expect.now - ID_TOKEN_MAX_AGE_SECS - ID_TOKEN_LEEWAY_SECS
    {
        return Err(IdTokenError::BadIssuedAt);
    }
    if expect.nonce.is_empty() || claims.nonce.as_deref() != Some(expect.nonce) {
        return Err(IdTokenError::BadNonce);
    }
    let email = claims
        .email
        .map(|e| e.trim().to_lowercase())
        .filter(|e| e.contains('@'))
        .ok_or(IdTokenError::EmailNotVerified)?;
    let verified = match expect.provider {
        "microsoft" => truthy(&claims.email_verified) || truthy(&claims.xms_edov),
        _ => truthy(&claims.email_verified),
    };
    if !verified {
        return Err(IdTokenError::EmailNotVerified);
    }
    Ok(VerifiedClaims {
        sub: claims.sub,
        email,
        name: claims.name,
    })
}

type JwksCache = Mutex<HashMap<String, (Instant, Arc<JwkSet>)>>;

fn jwks_cache() -> &'static JwksCache {
    static CACHE: OnceLock<JwksCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The provider's signing keys, cached for [`JWKS_TTL`] (`refresh` bypasses
/// the cache — used once when a token names a key we don't have).
async fn provider_jwks(
    http: &reqwest::Client,
    provider: &str,
    url: &str,
    refresh: bool,
) -> ApiResult<Arc<JwkSet>> {
    if !refresh {
        if let Some((at, set)) = jwks_cache()
            .lock()
            .ok()
            .and_then(|c| c.get(provider).cloned())
        {
            if at.elapsed() < JWKS_TTL {
                return Ok(set);
            }
        }
    }
    let raw: serde_json::Value = http
        .get(url)
        .send()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .error_for_status()
        .map_err(|e| ApiError::Internal(e.into()))?
        .json()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    let set = Arc::new(parse_jwks(&raw));
    if set.keys.is_empty() {
        return Err(ApiError::Internal(anyhow::anyhow!(
            "{provider} published no usable signing keys"
        )));
    }
    if let Ok(mut c) = jwks_cache().lock() {
        c.insert(provider.to_string(), (Instant::now(), set.clone()));
    }
    Ok(set)
}

/// Parse a JWKS document, skipping keys this library can't use (rather than
/// failing the whole set on one unfamiliar entry).
fn parse_jwks(raw: &serde_json::Value) -> JwkSet {
    let keys = raw
        .get("keys")
        .and_then(|k| k.as_array())
        .map(|ks| {
            ks.iter()
                .filter_map(|k| serde_json::from_value::<Jwk>(k.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    JwkSet { keys }
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
            sandbox_auth: false,
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

    #[test]
    fn mode_needs_live_or_an_explicitly_non_production_deployment() {
        assert_eq!(mode_for(true, false), Some(Mode::Live));
        assert_eq!(mode_for(true, true), Some(Mode::Live));
        assert_eq!(mode_for(false, true), Some(Mode::Sandbox));
        // Production with the provider not live: unavailable, never the sandbox.
        assert_eq!(mode_for(false, false), None);
    }

    fn prod_cfg() -> Config {
        cfg() // sandbox_auth: false
    }

    fn dev_cfg() -> Config {
        Config {
            sandbox_auth: true,
            ..cfg()
        }
    }

    fn state_for(cfg: &Config, provider: &str) -> String {
        let now = Utc::now().timestamp();
        sign_state(
            cfg,
            &StateClaims {
                typ: STATE_TYP.into(),
                provider: provider.into(),
                intent: "login".into(),
                tenant_id: None,
                link_user_id: None,
                pkce_verifier: "v".into(),
                nonce: "n".into(),
                iat: now,
                exp: now + 100,
            },
        )
        .unwrap()
    }

    #[test]
    fn sandbox_provider_is_refused_on_production() {
        // (The test process has no LIVE_PROVIDERS, so google is not live.)
        let prod = prod_cfg();
        assert_eq!(mode(&prod, "google"), None);
        let state = state_for(&prod, "google");
        let err = sandbox_redirect(&prod, "google", &state, Some("avery@acrehq.com")).unwrap_err();
        assert!(matches!(err, ApiError::NotFound(_)), "got {err:?}");

        // A dev deployment still gets the sandbox.
        let dev = dev_cfg();
        assert_eq!(mode(&dev, "google"), Some(Mode::Sandbox));
        let state = state_for(&dev, "google");
        let url = sandbox_redirect(&dev, "google", &state, Some("dev@x.com")).unwrap();
        assert!(url.contains("/auth/callback?provider=google&code="));
        // Unknown providers are never available.
        assert_eq!(mode(&dev, "myspace"), None);
    }

    #[rocket::async_test]
    async fn sandbox_codes_are_refused_on_production() {
        // Mint a sandbox code as a dev deployment would, then present it to a
        // production one (same signing key): the callback must refuse it.
        let dev = dev_cfg();
        let state = state_for(&dev, "google");
        let redirect = sandbox_redirect(&dev, "google", &state, Some("avery@acrehq.com")).unwrap();
        let code = redirect
            .split("code=")
            .nth(1)
            .and_then(|r| r.split('&').next())
            .unwrap()
            .to_string();
        let db = sea_orm::DatabaseConnection::default();
        let err = exchange(&prod_cfg(), &db, "google", &code, &state)
            .await
            .err()
            .expect("production must refuse a sandbox code");
        assert!(matches!(err, ApiError::Forbidden(_)), "got {err:?}");
        let start_err = start(&prod_cfg(), &db, "google", "login", None, None)
            .await
            .err()
            .expect("production must refuse to start a sandbox flow");
        assert!(
            matches!(start_err, ApiError::Forbidden(_)),
            "got {start_err:?}"
        );
        // The same code is accepted by the dev deployment.
        let (id, _) = exchange(&dev, &db, "google", &code, &state).await.unwrap();
        assert_eq!(id.email, "avery@acrehq.com");
    }

    // ---- ID-token verification -------------------------------------------

    use p256::ecdsa::SigningKey;
    use p256::pkcs8::{EncodePrivateKey, LineEnding};

    const CLIENT: &str = "client-123.apps.example";
    const NONCE: &str = "nonce-abc";

    struct TestKey {
        enc: EncodingKey,
        jwks: JwkSet,
    }

    fn test_key(kid: &str) -> TestKey {
        let sk = SigningKey::random(&mut rand::rngs::OsRng);
        let pem = sk.to_pkcs8_pem(LineEnding::LF).unwrap();
        let enc = EncodingKey::from_ec_pem(pem.as_bytes()).unwrap();
        let point = sk.verifying_key().to_encoded_point(false);
        let b64 = |b: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
        let raw = serde_json::json!({ "keys": [
            // An entry the library can't use is skipped, not fatal.
            { "kty": "weird", "kid": "junk" },
            {
                "kty": "EC", "crv": "P-256", "use": "sig", "alg": "ES256", "kid": kid,
                "x": b64(point.x().unwrap()), "y": b64(point.y().unwrap()),
            }
        ]});
        let jwks = parse_jwks(&raw);
        assert_eq!(jwks.keys.len(), 1);
        TestKey { enc, jwks }
    }

    fn good_claims(now: i64) -> serde_json::Value {
        serde_json::json!({
            "iss": "https://accounts.google.com",
            "aud": CLIENT,
            "sub": "1234567890",
            "iat": now,
            "exp": now + 3600,
            "nonce": NONCE,
            "email": "Avery@AcreHQ.com",
            "email_verified": true,
            "name": "Avery Stone",
        })
    }

    fn sign(key: &TestKey, kid: &str, claims: &serde_json::Value) -> String {
        let mut h = Header::new(Algorithm::ES256);
        h.kid = Some(kid.into());
        encode(&h, claims, &key.enc).unwrap()
    }

    fn expect(provider: &str, now: i64) -> Expected<'_> {
        Expected {
            provider,
            client_id: CLIENT,
            nonce: NONCE,
            now,
        }
    }

    fn check(key: &TestKey, claims: serde_json::Value) -> Result<VerifiedClaims, IdTokenError> {
        let now = Utc::now().timestamp();
        verify_id_token(&sign(key, "k1", &claims), &key.jwks, &expect("google", now))
    }

    fn with(mut c: serde_json::Value, k: &str, v: serde_json::Value) -> serde_json::Value {
        c[k] = v;
        c
    }

    fn without(mut c: serde_json::Value, k: &str) -> serde_json::Value {
        c.as_object_mut().unwrap().remove(k);
        c
    }

    #[test]
    fn a_valid_id_token_verifies() {
        let key = test_key("k1");
        let now = Utc::now().timestamp();
        let v = check(&key, good_claims(now)).unwrap();
        assert_eq!(v.sub, "1234567890");
        assert_eq!(v.email, "avery@acrehq.com");
        // `accounts.google.com` (no scheme) is also a Google issuer.
        let alt = with(good_claims(now), "iss", "accounts.google.com".into());
        assert!(check(&key, alt).is_ok());
        // Apple sends email_verified as a string.
        let apple = with(
            with(good_claims(now), "iss", "https://appleid.apple.com".into()),
            "email_verified",
            "true".into(),
        );
        let t = sign(&key, "k1", &apple);
        assert!(verify_id_token(&t, &key.jwks, &expect("apple", now)).is_ok());
    }

    #[test]
    fn id_token_signed_by_another_key_is_refused() {
        let key = test_key("k1");
        let attacker = test_key("k1"); // same kid, different key
        let now = Utc::now().timestamp();
        let forged = sign(&attacker, "k1", &good_claims(now));
        let err = verify_id_token(&forged, &key.jwks, &expect("google", now)).unwrap_err();
        assert!(matches!(err, IdTokenError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn id_token_with_unknown_kid_is_refused() {
        let key = test_key("k1");
        let now = Utc::now().timestamp();
        let t = sign(&key, "other", &good_claims(now));
        assert_eq!(
            verify_id_token(&t, &key.jwks, &expect("google", now)).unwrap_err(),
            IdTokenError::UnknownKey
        );
    }

    #[test]
    fn id_token_with_symmetric_or_no_algorithm_is_refused() {
        let key = test_key("k1");
        let now = Utc::now().timestamp();
        let mut h = Header::new(Algorithm::HS256);
        h.kid = Some("k1".into());
        let t = encode(&h, &good_claims(now), &EncodingKey::from_secret(b"guess")).unwrap();
        assert_eq!(
            verify_id_token(&t, &key.jwks, &expect("google", now)).unwrap_err(),
            IdTokenError::BadAlgorithm
        );
        assert_eq!(
            verify_id_token("not-a-jwt", &key.jwks, &expect("google", now)).unwrap_err(),
            IdTokenError::Malformed
        );
    }

    #[test]
    fn id_token_for_another_client_is_refused() {
        let key = test_key("k1");
        let now = Utc::now().timestamp();
        let err = check(&key, with(good_claims(now), "aud", "someone-else".into())).unwrap_err();
        assert!(matches!(err, IdTokenError::Invalid(_)), "got {err:?}");
        let err = check(&key, without(good_claims(now), "aud")).unwrap_err();
        assert!(matches!(err, IdTokenError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn id_token_from_the_wrong_issuer_is_refused() {
        let key = test_key("k1");
        let now = Utc::now().timestamp();
        let c = with(good_claims(now), "iss", "https://evil.example".into());
        assert_eq!(check(&key, c).unwrap_err(), IdTokenError::BadIssuer);
        // A Google token is not an Apple token.
        let t = sign(&key, "k1", &good_claims(now));
        assert_eq!(
            verify_id_token(&t, &key.jwks, &expect("apple", now)).unwrap_err(),
            IdTokenError::BadIssuer
        );
    }

    #[test]
    fn expired_or_badly_timed_id_token_is_refused() {
        let key = test_key("k1");
        let now = Utc::now().timestamp();
        let expired = with(good_claims(now - 7200), "exp", (now - 3600).into());
        assert!(matches!(
            check(&key, expired).unwrap_err(),
            IdTokenError::Invalid(_)
        ));
        let future = with(good_claims(now), "iat", (now + 3600).into());
        assert_eq!(check(&key, future).unwrap_err(), IdTokenError::BadIssuedAt);
        let stale = with(good_claims(now), "iat", (now - 3 * 3600).into());
        assert_eq!(check(&key, stale).unwrap_err(), IdTokenError::BadIssuedAt);
        assert!(matches!(
            check(&key, without(good_claims(now), "iat")).unwrap_err(),
            IdTokenError::Invalid(_)
        ));
    }

    #[test]
    fn id_token_with_wrong_or_missing_nonce_is_refused() {
        let key = test_key("k1");
        let now = Utc::now().timestamp();
        let c = with(good_claims(now), "nonce", "replayed".into());
        assert_eq!(check(&key, c).unwrap_err(), IdTokenError::BadNonce);
        let c = without(good_claims(now), "nonce");
        assert_eq!(check(&key, c).unwrap_err(), IdTokenError::BadNonce);
    }

    #[test]
    fn id_token_without_a_verified_email_is_refused() {
        let key = test_key("k1");
        let now = Utc::now().timestamp();
        let c = with(good_claims(now), "email_verified", false.into());
        assert_eq!(check(&key, c).unwrap_err(), IdTokenError::EmailNotVerified);
        let c = without(good_claims(now), "email_verified");
        assert_eq!(check(&key, c).unwrap_err(), IdTokenError::EmailNotVerified);
        let c = without(good_claims(now), "email");
        assert_eq!(check(&key, c).unwrap_err(), IdTokenError::EmailNotVerified);
    }

    #[test]
    fn microsoft_issuer_must_match_its_directory() {
        let key = test_key("k1");
        let now = Utc::now().timestamp();
        let tid = "9188040d-6c67-4c5b-b112-36a304b66dad";
        let ms = serde_json::json!({
            "iss": format!("https://login.microsoftonline.com/{tid}/v2.0"),
            "tid": tid,
            "aud": CLIENT, "sub": "ms-sub", "iat": now, "exp": now + 3600,
            "nonce": NONCE, "email": "jo@contoso.com", "xms_edov": true,
        });
        let t = sign(&key, "k1", &ms);
        assert!(verify_id_token(&t, &key.jwks, &expect("microsoft", now)).is_ok());
        // Issuer for a different directory than the token claims.
        let bad = with(
            ms.clone(),
            "tid",
            "00000000-0000-0000-0000-000000000000".into(),
        );
        let t = sign(&key, "k1", &bad);
        assert_eq!(
            verify_id_token(&t, &key.jwks, &expect("microsoft", now)).unwrap_err(),
            IdTokenError::BadIssuer
        );
        // No verified-email signal from Entra: refused.
        let t = sign(&key, "k1", &without(ms, "xms_edov"));
        assert_eq!(
            verify_id_token(&t, &key.jwks, &expect("microsoft", now)).unwrap_err(),
            IdTokenError::EmailNotVerified
        );
    }
}
