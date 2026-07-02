//! Runtime configuration, loaded from environment variables (and `.env`).

use sha2::{Digest, Sha256};
use std::env;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct Config {
    /// Postgres connection string.
    pub database_url: String,
    /// HMAC secret for signing JWT access tokens. **Override in production.**
    pub jwt_secret: String,
    /// 32-byte key for AES-256-GCM PII encryption (SSN / government IDs).
    pub pii_key: Vec<u8>,
    /// 32-byte key for AES-256-GCM integration-credential encryption. Distinct
    /// from `pii_key` so a leaked provider credential and a leaked SSN stay
    /// independently rotatable blast radii.
    pub secrets_key: Vec<u8>,
    /// Access-token lifetime in seconds (default 15 min).
    pub access_ttl_secs: i64,
    /// Refresh-token lifetime in seconds (default 30 days).
    pub refresh_ttl_secs: i64,
    /// Whether to run migrations + seed on boot (handy in dev).
    pub auto_migrate: bool,
}

static GLOBAL: OnceLock<Config> = OnceLock::new();

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "dev-insecure-change-me".into());
        Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://localhost:5432/acre".into()),
            pii_key: pii_key_from_env(&jwt_secret),
            secrets_key: secrets_key_from_env(&jwt_secret),
            jwt_secret,
            access_ttl_secs: env::var("ACCESS_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(900),
            refresh_ttl_secs: env::var("REFRESH_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60 * 60 * 24 * 30),
            auto_migrate: env::var("AUTO_MIGRATE")
                .map(|v| v != "0" && v.to_lowercase() != "false")
                .unwrap_or(true),
        }
    }

    /// The process-wide configuration, loaded once. Used by code that runs with
    /// no request/state in scope (the background scheduler's job handlers).
    pub fn global() -> &'static Config {
        GLOBAL.get_or_init(Config::from_env)
    }
}

/// True when the process declares itself a production deployment
/// (`APP_ENV=production`/`prod`). Key-resolution fails closed in that case.
fn is_production() -> bool {
    matches!(
        env::var("APP_ENV").ok().as_deref(),
        Some("production") | Some("prod")
    )
}

/// Resolve the 32-byte PII encryption key. Prefer `PII_ENC_KEY` (64 hex chars);
/// otherwise derive a stable key from the JWT secret and warn loudly — fine for
/// dev, but production must set an independent, KMS-managed key.
fn pii_key_from_env(jwt_secret: &str) -> Vec<u8> {
    if let Ok(hex) = env::var("PII_ENC_KEY") {
        if let Ok(bytes) = hex_decode_32(&hex) {
            return bytes;
        }
        tracing::warn!("PII_ENC_KEY is not 64 hex chars; falling back to a derived key");
    } else {
        tracing::warn!("PII_ENC_KEY not set; deriving PII key from JWT_SECRET (dev only)");
    }
    // Derive 32 bytes deterministically so existing ciphertext stays decryptable.
    derived_key(b"acre-pii-v1:", jwt_secret)
}

/// Resolve the 32-byte integration-secrets key from `SECRETS_ENC_KEY` (64 hex
/// chars). Unlike the PII key's dev-era fallback, this **fails closed in
/// production** from day one: a prod boot without an explicit key panics
/// instead of silently deriving one from `JWT_SECRET`.
fn secrets_key_from_env(jwt_secret: &str) -> Vec<u8> {
    match env::var("SECRETS_ENC_KEY") {
        Ok(hex) => {
            if let Ok(bytes) = hex_decode_32(&hex) {
                return bytes;
            }
            if is_production() {
                panic!("SECRETS_ENC_KEY must be 64 hex chars (32 bytes) in production");
            }
            tracing::warn!("SECRETS_ENC_KEY is not 64 hex chars; deriving a dev-only key");
        }
        Err(_) => {
            if is_production() {
                panic!(
                    "SECRETS_ENC_KEY is required in production (APP_ENV=production); \
                     refusing to derive the integration-secrets key from JWT_SECRET"
                );
            }
            tracing::warn!(
                "SECRETS_ENC_KEY not set; deriving secrets key from JWT_SECRET (dev only)"
            );
        }
    }
    derived_key(b"acre-secrets-v1:", jwt_secret)
}

/// Derive 32 bytes deterministically from a domain-separated hash of the JWT
/// secret. Dev-only convenience — production sets explicit keys.
fn derived_key(domain: &[u8], jwt_secret: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(domain);
    h.update(jwt_secret.as_bytes());
    h.finalize().to_vec()
}

fn hex_decode_32(s: &str) -> Result<Vec<u8>, ()> {
    let s = s.trim();
    if s.len() != 64 {
        return Err(());
    }
    (0..32)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| ()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pii_and_secrets_dev_keys_are_domain_separated() {
        let pii = derived_key(b"acre-pii-v1:", "same-secret");
        let secrets = derived_key(b"acre-secrets-v1:", "same-secret");
        assert_eq!(pii.len(), 32);
        assert_eq!(secrets.len(), 32);
        assert_ne!(pii, secrets, "the two derived keys must never coincide");
    }

    #[test]
    fn hex_decode_32_roundtrip() {
        let hex = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
        let bytes = hex_decode_32(hex).unwrap();
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes[0], 0x00);
        assert_eq!(bytes[31], 0xff);
        assert!(hex_decode_32("deadbeef").is_err());
    }
}
