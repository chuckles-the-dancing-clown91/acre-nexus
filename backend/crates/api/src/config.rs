//! Runtime configuration, loaded from environment variables (and `.env`).
//!
//! Since the database split there are **three** databases (user / property /
//! client). Each has a *runtime* URL (connect as the least-privilege `_app`
//! role so row-level-security bites) and an *owner* URL (used only to run
//! migrations on boot, which need DDL privileges). All of them fall back to a
//! shared `DATABASE_URL`, so a simple single-database dev setup still works.

use sha2::{Digest, Sha256};
use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    /// Runtime connection URL for the `acre_user` database (identity/auth/RBAC/
    /// tenancy + audit_log + background_job). Connect as the non-owner `_app` role.
    pub user_db_url: String,
    /// Runtime connection URL for the `acre_property` database.
    pub property_db_url: String,
    /// Runtime connection URL for the `acre_client` database.
    pub client_db_url: String,
    /// Owner (DDL) URL for `acre_user`, used to run migrations on boot.
    pub user_owner_url: String,
    /// Owner (DDL) URL for `acre_property`.
    pub property_owner_url: String,
    /// Owner (DDL) URL for `acre_client`.
    pub client_owner_url: String,
    /// HMAC secret for signing JWT access tokens. **Override in production.**
    pub jwt_secret: String,
    /// 32-byte key for AES-256-GCM PII encryption (SSN / government IDs).
    pub pii_key: Vec<u8>,
    /// Access-token lifetime in seconds (default 15 min).
    pub access_ttl_secs: i64,
    /// Refresh-token lifetime in seconds (default 30 days).
    pub refresh_ttl_secs: i64,
    /// Whether to run migrations + seed on boot (handy in dev).
    pub auto_migrate: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "dev-insecure-change-me".into());
        Config {
            user_db_url: domain_url("USER"),
            property_db_url: domain_url("PROPERTY"),
            client_db_url: domain_url("CLIENT"),
            user_owner_url: owner_url("USER"),
            property_owner_url: owner_url("PROPERTY"),
            client_owner_url: owner_url("CLIENT"),
            pii_key: pii_key_from_env(&jwt_secret),
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
}

/// Runtime URL for a domain database: `<DOMAIN>_DATABASE_URL`, else the shared
/// `DATABASE_URL`, else a localhost default.
fn domain_url(domain: &str) -> String {
    env::var(format!("{domain}_DATABASE_URL"))
        .or_else(|_| env::var("DATABASE_URL"))
        .unwrap_or_else(|_| format!("postgres://localhost:5432/acre_{}", domain.to_lowercase()))
}

/// Owner (DDL) URL for migrations: `<DOMAIN>_DATABASE_OWNER_URL`, else the
/// domain's runtime URL (which itself falls back to `DATABASE_URL`).
fn owner_url(domain: &str) -> String {
    env::var(format!("{domain}_DATABASE_OWNER_URL")).unwrap_or_else(|_| domain_url(domain))
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
    let mut h = Sha256::new();
    h.update(b"acre-pii-v1:");
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
