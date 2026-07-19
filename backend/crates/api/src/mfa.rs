//! **MFA** support (issue #63): the short-lived *challenge token* minted after
//! a first factor succeeds on an MFA-enabled account (exchanged, with a valid
//! TOTP code, for a real session at `POST /auth/mfa/verify`), plus sealing of
//! the TOTP secret at rest. The one-time-code engine itself is [`crate::totp`].

use crate::config::Config;
use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A challenge token is valid for five minutes — long enough to open an
/// authenticator app, short enough to limit replay.
const CHALLENGE_TTL_SECS: i64 = 300;
const CHALLENGE_TYP: &str = "mfa_challenge";

#[derive(Serialize, Deserialize)]
struct ChallengeClaims {
    sub: Uuid,
    /// Discriminator so a challenge token can never be used as an access token
    /// (or vice-versa) despite sharing the signing key.
    typ: String,
    iat: i64,
    exp: i64,
}

/// Mint a challenge token binding a pending login to `user_id`.
pub fn issue_challenge_token(cfg: &Config, user_id: Uuid) -> anyhow::Result<String> {
    let now = Utc::now().timestamp();
    let claims = ChallengeClaims {
        sub: user_id,
        typ: CHALLENGE_TYP.into(),
        iat: now,
        exp: now + CHALLENGE_TTL_SECS,
    };
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )?)
}

/// Recover the user id from a valid, unexpired challenge token. `None` if the
/// token is malformed, expired, tampered, or not a challenge token.
pub fn verify_challenge_token(cfg: &Config, token: &str) -> Option<Uuid> {
    let data = decode::<ChallengeClaims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .ok()?;
    (data.claims.typ == CHALLENGE_TYP).then_some(data.claims.sub)
}

/// Seal a base32 TOTP secret for storage → `(ciphertext, nonce)` (AES-256-GCM
/// under the PII key; never persisted in plaintext).
pub fn seal_secret(cfg: &Config, secret_b32: &str) -> anyhow::Result<(String, String)> {
    let sealed = crate::pii::encrypt(&cfg.pii_key, secret_b32)?;
    Ok((sealed.ciphertext, sealed.nonce))
}

/// Recover a sealed TOTP secret.
pub fn open_secret(cfg: &Config, ciphertext: &str, nonce: &str) -> anyhow::Result<String> {
    crate::pii::decrypt(&cfg.pii_key, ciphertext, nonce)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cfg() -> Config {
        // A config with fixed keys is enough for the token + seal round-trips.
        Config {
            database_url: String::new(),
            jwt_secret: "test-secret-please-ignore-0123456789".into(),
            pii_key: vec![7u8; 32],
            secrets_key: vec![9u8; 32],
            access_ttl_secs: 900,
            refresh_ttl_secs: 1000,
            auto_migrate: false,
        }
    }

    #[test]
    fn challenge_token_roundtrips_and_is_typed() {
        let cfg = test_cfg();
        let uid = Uuid::new_v4();
        let token = issue_challenge_token(&cfg, uid).unwrap();
        assert_eq!(verify_challenge_token(&cfg, &token), Some(uid));
        // Garbage / tampered tokens are rejected.
        assert_eq!(verify_challenge_token(&cfg, "not.a.jwt"), None);
        assert_eq!(verify_challenge_token(&cfg, &format!("{token}x")), None);
    }

    #[test]
    fn secret_seal_roundtrips() {
        let cfg = test_cfg();
        let secret = crate::totp::generate_secret();
        let (ct, nonce) = seal_secret(&cfg, &secret).unwrap();
        assert_ne!(ct, secret);
        assert_eq!(open_secret(&cfg, &ct, &nonce).unwrap(), secret);
    }
}
