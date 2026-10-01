//! **Alpha ↔ Vantedge single sign-on.** A signed, short-lived assertion that
//! says "this person, at this email, is signed in on the other side". No
//! shared database and no shared sessions: each side verifies the assertion
//! with a per-workspace shared secret and mints its own session.
//!
//! * HS256 JWT with `iss`, `aud`, `sub` (email), `tenant` (workspace slug),
//!   `jti`, `iat` and `exp` (never more than two minutes after `iat`).
//! * Vantedge accepts `iss = alpha`, `aud = vantedge`; it issues
//!   `iss = vantedge`, `aud = alpha`.
//! * The `jti` is stored on first use, so a token cannot be replayed.
//! * The person must already exist and belong to the workspace. Nothing is
//!   provisioned by a sign-in token. Two-step sign-in is still required.

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// The longest an assertion may live.
pub const MAX_TTL_SECS: i64 = 120;
/// Clock skew tolerated between the two servers.
const LEEWAY_SECS: u64 = 5;

pub const ISS_VANTEDGE: &str = "vantedge";
pub const ISS_ALPHA: &str = "alpha";

/// The vault key holding the workspace's shared secret.
pub const SECRET_KEY: &str = "sso.alpha.secret";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    pub iss: String,
    pub aud: String,
    /// The person's email.
    pub sub: String,
    /// The workspace slug.
    pub tenant: String,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
    /// Where to land afterwards, a path on the receiving side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
}

/// Sign an assertion.
pub fn sign(secret: &str, claims: &Claims) -> Result<String, String> {
    encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| e.to_string())
}

/// Build claims valid for `ttl` seconds from `now`.
pub fn claims(
    iss: &str,
    aud: &str,
    email: &str,
    tenant: &str,
    next: Option<String>,
    now: i64,
    ttl: i64,
) -> Claims {
    Claims {
        iss: iss.into(),
        aud: aud.into(),
        sub: email.trim().to_lowercase(),
        tenant: tenant.into(),
        jti: uuid::Uuid::new_v4().simple().to_string(),
        iat: now,
        exp: now + ttl.clamp(1, MAX_TTL_SECS),
        next,
    }
}

/// The `tenant` claim of a token, read without trusting it, so the right
/// secret can be fetched. Everything is verified afterwards.
pub fn peek_tenant(token: &str) -> Option<String> {
    use base64::Engine;
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()?
        .get("tenant")?
        .as_str()
        .map(str::to_string)
}

/// Verify signature, issuer, audience, expiry and lifetime. `now` is passed in
/// so the lifetime rule is testable.
pub fn verify(token: &str, secret: &str, iss: &str, aud: &str, now: i64) -> Result<Claims, String> {
    let mut v = Validation::new(Algorithm::HS256);
    v.set_audience(&[aud]);
    v.set_issuer(&[iss]);
    v.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    v.leeway = LEEWAY_SECS;
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &v)
        .map_err(|e| format!("invalid sign-in token: {e}"))?;
    let c = data.claims;
    if c.exp - c.iat > MAX_TTL_SECS {
        return Err("sign-in token lives too long".into());
    }
    if c.iat > now + LEEWAY_SECS as i64 {
        return Err("sign-in token is from the future".into());
    }
    if c.jti.trim().is_empty() {
        return Err("sign-in token has no id".into());
    }
    if !c.sub.contains('@') {
        return Err("sign-in token has no email".into());
    }
    Ok(c)
}

/// Only same-site paths survive as a landing target.
pub fn safe_next(next: Option<&str>) -> Option<String> {
    let n = next?.trim();
    (n.starts_with('/') && !n.starts_with("//") && !n.contains("://") && !n.contains('\\'))
        .then(|| n.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    fn good() -> Claims {
        claims(
            ISS_ALPHA,
            "vantedge",
            "Pat@Example.com",
            "northwind",
            None,
            NOW,
            60,
        )
    }

    #[test]
    fn round_trip() {
        let t = sign("s3cret", &good()).unwrap();
        let c = verify(&t, "s3cret", ISS_ALPHA, "vantedge", NOW + 1).unwrap();
        assert_eq!(c.sub, "pat@example.com");
        assert_eq!(c.tenant, "northwind");
        assert_eq!(peek_tenant(&t).as_deref(), Some("northwind"));
    }

    #[test]
    fn wrong_secret_issuer_or_audience_is_refused() {
        let t = sign("s3cret", &good()).unwrap();
        assert!(verify(&t, "other", ISS_ALPHA, "vantedge", NOW).is_err());
        assert!(verify(&t, "s3cret", "someone", "vantedge", NOW).is_err());
        assert!(verify(&t, "s3cret", ISS_ALPHA, "alpha", NOW).is_err());
    }

    #[test]
    fn expired_long_lived_and_future_tokens_are_refused() {
        // A token from 2001 has long expired.
        let old = claims(
            ISS_ALPHA,
            "vantedge",
            "a@b.co",
            "t",
            None,
            1_000_000_000,
            60,
        );
        let t = sign("s3cret", &old).unwrap();
        assert!(verify(&t, "s3cret", ISS_ALPHA, "vantedge", 1_000_000_001).is_err());
        let now = chrono::Utc::now().timestamp();
        let mut c = claims(ISS_ALPHA, "vantedge", "a@b.co", "t", None, now, 60);
        c.exp = now + 3600;
        let t = sign("s", &c).unwrap();
        assert!(verify(&t, "s", ISS_ALPHA, "vantedge", now)
            .unwrap_err()
            .contains("too long"));
        let mut c = claims(ISS_ALPHA, "vantedge", "a@b.co", "t", None, now + 600, 60);
        c.exp = now + 660;
        let t = sign("s", &c).unwrap();
        assert!(verify(&t, "s", ISS_ALPHA, "vantedge", now).is_err());
        let fresh = claims(ISS_ALPHA, "vantedge", "a@b.co", "t", None, now, 60);
        let t = sign("s", &fresh).unwrap();
        assert!(verify(&t, "s", ISS_ALPHA, "vantedge", now).is_ok());
    }

    #[test]
    fn tampering_and_junk_are_refused() {
        let now = chrono::Utc::now().timestamp();
        let t = sign(
            "s",
            &claims(ISS_ALPHA, "vantedge", "a@b.co", "t", None, now, 60),
        )
        .unwrap();
        let mut parts: Vec<&str> = t.split('.').collect();
        parts[1] = "eyJzdWIiOiJldmlsQGV4YW1wbGUuY29tIn0";
        assert!(verify(&parts.join("."), "s", ISS_ALPHA, "vantedge", now).is_err());
        assert!(verify("not a token", "s", ISS_ALPHA, "vantedge", now).is_err());
        assert!(peek_tenant("a.b").is_none());
    }

    #[test]
    fn landing_paths_stay_on_site() {
        assert_eq!(
            safe_next(Some("/console/maintenance")).as_deref(),
            Some("/console/maintenance")
        );
        assert!(safe_next(Some("//evil.com")).is_none());
        assert!(safe_next(Some("https://evil.com")).is_none());
        assert!(safe_next(Some("/\\evil")).is_none());
        assert!(safe_next(Some("evil")).is_none());
        assert!(safe_next(None).is_none());
    }
}
