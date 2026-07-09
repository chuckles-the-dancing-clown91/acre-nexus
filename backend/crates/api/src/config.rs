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

/// The insecure default JWT secret shipped in `.env.example`. Fine for local
/// dev; rejected outright in production so a copy-pasted dev config can never
/// sign real access tokens with a publicly-known key.
const DEV_JWT_SECRET: &str = "dev-insecure-change-me";

/// Minimum acceptable `JWT_SECRET` length (characters) in production. Anything
/// shorter is treated as too weak to HMAC access tokens with. `openssl rand
/// -hex 32` (64 chars) or `-base64 32` (44 chars) both clear this comfortably.
const MIN_JWT_SECRET_LEN: usize = 32;

impl Config {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let production = is_production();
        // Each secret/key fails closed in production: an explicit, strong value
        // is required, and startup panics with a clear message otherwise (rather
        // than silently degrading to a dev default or a value derived from
        // `JWT_SECRET`). See issues #23/#24/#25.
        let jwt_secret = resolve_jwt_secret(env::var("JWT_SECRET").ok().as_deref(), production)
            .unwrap_or_else(|e| panic!("{e}"));
        let pii_key =
            resolve_pii_key(env::var("PII_ENC_KEY").ok().as_deref(), &jwt_secret, production)
                .unwrap_or_else(|e| panic!("{e}"));
        let secrets_key = resolve_secrets_key(
            env::var("SECRETS_ENC_KEY").ok().as_deref(),
            &jwt_secret,
            production,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://localhost:5432/acre".into()),
            pii_key,
            secrets_key,
            jwt_secret,
            access_ttl_secs: env::var("ACCESS_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(900),
            refresh_ttl_secs: env::var("REFRESH_TTL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60 * 60 * 24 * 30),
            auto_migrate: resolve_auto_migrate(env::var("AUTO_MIGRATE").ok().as_deref(), production),
        }
    }

    /// The process-wide configuration, loaded once. Used by code that runs with
    /// no request/state in scope (the background scheduler's job handlers).
    pub fn global() -> &'static Config {
        GLOBAL.get_or_init(Config::from_env)
    }
}

/// True when the process declares itself a production deployment
/// (`APP_ENV=production`/`prod`). Key-resolution fails closed in that case, and
/// auto-migrate/seed default off (see [`resolve_auto_migrate`] and `seed::run`).
pub(crate) fn is_production() -> bool {
    matches!(
        env::var("APP_ENV").ok().as_deref(),
        Some("production") | Some("prod")
    )
}

/// Resolve the JWT signing secret. In production an explicit secret is
/// **required** — startup fails if it is unset/empty, still the known
/// `.env.example` default, or below [`MIN_JWT_SECRET_LEN`]. In dev an insecure
/// default is used for a friction-free boot.
fn resolve_jwt_secret(var: Option<&str>, production: bool) -> Result<String, String> {
    match var {
        Some(secret) if !secret.trim().is_empty() => {
            if production {
                if secret == DEV_JWT_SECRET {
                    return Err("JWT_SECRET is still the insecure .env.example default in \
                                production (APP_ENV=production); set a strong, unique secret \
                                (`openssl rand -hex 32`)"
                        .into());
                }
                if secret.len() < MIN_JWT_SECRET_LEN {
                    return Err(format!(
                        "JWT_SECRET must be at least {MIN_JWT_SECRET_LEN} characters in \
                         production (APP_ENV=production); generate one with `openssl rand -hex 32`"
                    ));
                }
            }
            Ok(secret.to_string())
        }
        _ => {
            if production {
                return Err("JWT_SECRET is required in production (APP_ENV=production); \
                            refusing to sign access tokens with a built-in dev default"
                    .into());
            }
            tracing::warn!("JWT_SECRET not set; using an insecure dev default");
            Ok(DEV_JWT_SECRET.to_string())
        }
    }
}

/// Resolve the 32-byte PII encryption key. Prefer `PII_ENC_KEY` (64 hex chars).
/// **Fails closed in production**: a prod boot without a valid explicit key
/// panics instead of silently deriving one from `JWT_SECRET`. In dev it derives
/// a stable key from the JWT secret so existing ciphertext stays decryptable.
fn resolve_pii_key(
    var: Option<&str>,
    jwt_secret: &str,
    production: bool,
) -> Result<Vec<u8>, String> {
    match var {
        Some(hex) if !hex.trim().is_empty() => {
            if let Ok(bytes) = hex_decode_32(hex) {
                return Ok(bytes);
            }
            if production {
                return Err("PII_ENC_KEY must be 64 hex chars (32 bytes) in production \
                            (APP_ENV=production)"
                    .into());
            }
            tracing::warn!("PII_ENC_KEY is not 64 hex chars; deriving a dev-only key");
        }
        _ => {
            if production {
                return Err("PII_ENC_KEY is required in production (APP_ENV=production); \
                            refusing to derive the PII key from JWT_SECRET"
                    .into());
            }
            tracing::warn!("PII_ENC_KEY not set; deriving PII key from JWT_SECRET (dev only)");
        }
    }
    // Derive 32 bytes deterministically so existing ciphertext stays decryptable.
    Ok(derived_key(b"acre-pii-v1:", jwt_secret))
}

/// Resolve the 32-byte integration-secrets key from `SECRETS_ENC_KEY` (64 hex
/// chars). Like the PII key, this **fails closed in production**: a prod boot
/// without an explicit key panics instead of deriving one from `JWT_SECRET`.
fn resolve_secrets_key(
    var: Option<&str>,
    jwt_secret: &str,
    production: bool,
) -> Result<Vec<u8>, String> {
    match var {
        Some(hex) if !hex.trim().is_empty() => {
            if let Ok(bytes) = hex_decode_32(hex) {
                return Ok(bytes);
            }
            if production {
                return Err("SECRETS_ENC_KEY must be 64 hex chars (32 bytes) in production \
                            (APP_ENV=production)"
                    .into());
            }
            tracing::warn!("SECRETS_ENC_KEY is not 64 hex chars; deriving a dev-only key");
        }
        _ => {
            if production {
                return Err("SECRETS_ENC_KEY is required in production (APP_ENV=production); \
                            refusing to derive the integration-secrets key from JWT_SECRET"
                    .into());
            }
            tracing::warn!("SECRETS_ENC_KEY not set; deriving secrets key from JWT_SECRET (dev only)");
        }
    }
    Ok(derived_key(b"acre-secrets-v1:", jwt_secret))
}

/// Resolve the auto-migrate/seed-on-boot flag. When `AUTO_MIGRATE` is set it is
/// honoured (`0`/`false` → off, anything else → on). When **unset** it defaults
/// **on in dev** (friction-free demo boot) and **off in production**, so a prod
/// environment never migrates or re-seeds unattended (issue #23). An operator
/// who really wants boot migrations in prod opts in explicitly with
/// `AUTO_MIGRATE=1`.
fn resolve_auto_migrate(var: Option<&str>, production: bool) -> bool {
    match var {
        Some(v) => v != "0" && !v.eq_ignore_ascii_case("false"),
        None => !production,
    }
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

    // A valid 64-hex-char (32-byte) key for the key-resolution tests.
    const VALID_HEX_KEY: &str =
        "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    // A production-strength JWT secret (>= MIN_JWT_SECRET_LEN, not the default).
    const STRONG_JWT: &str = "a-strong-unique-production-secret-value";

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
        let bytes = hex_decode_32(VALID_HEX_KEY).unwrap();
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes[0], 0x00);
        assert_eq!(bytes[31], 0xff);
        assert!(hex_decode_32("deadbeef").is_err());
    }

    // ---- #25: JWT secret fails closed in production ----

    #[test]
    fn jwt_secret_dev_defaults_when_unset() {
        assert_eq!(resolve_jwt_secret(None, false).unwrap(), DEV_JWT_SECRET);
        assert_eq!(resolve_jwt_secret(Some(""), false).unwrap(), DEV_JWT_SECRET);
        // Dev accepts even the weak default and short values.
        assert_eq!(resolve_jwt_secret(Some("short"), false).unwrap(), "short");
    }

    #[test]
    fn jwt_secret_required_and_strong_in_prod() {
        // Unset / empty → refuse to boot.
        assert!(resolve_jwt_secret(None, true).is_err());
        assert!(resolve_jwt_secret(Some("   "), true).is_err());
        // The known .env.example default is rejected.
        assert!(resolve_jwt_secret(Some(DEV_JWT_SECRET), true).is_err());
        // Too short is rejected.
        assert!(resolve_jwt_secret(Some("too-short-secret"), true).is_err());
        // A strong, unique secret is accepted.
        assert_eq!(resolve_jwt_secret(Some(STRONG_JWT), true).unwrap(), STRONG_JWT);
    }

    // ---- #24: PII key fails closed in production ----

    #[test]
    fn pii_key_derives_in_dev_fails_closed_in_prod() {
        // Dev: derives from JWT secret when unset or malformed.
        assert_eq!(resolve_pii_key(None, "s", false).unwrap().len(), 32);
        assert_eq!(resolve_pii_key(Some("nothex"), "s", false).unwrap().len(), 32);
        // Prod: an explicit valid key is accepted...
        let bytes = resolve_pii_key(Some(VALID_HEX_KEY), "s", true).unwrap();
        assert_eq!(bytes.len(), 32);
        // ...but a missing or malformed key refuses to boot.
        assert!(resolve_pii_key(None, "s", true).is_err());
        assert!(resolve_pii_key(Some(""), "s", true).is_err());
        assert!(resolve_pii_key(Some("nothex"), "s", true).is_err());
    }

    // ---- integration-secrets key parity ----

    #[test]
    fn secrets_key_derives_in_dev_fails_closed_in_prod() {
        assert_eq!(resolve_secrets_key(None, "s", false).unwrap().len(), 32);
        assert_eq!(resolve_secrets_key(Some(VALID_HEX_KEY), "s", true).unwrap().len(), 32);
        assert!(resolve_secrets_key(None, "s", true).is_err());
        assert!(resolve_secrets_key(Some("nothex"), "s", true).is_err());
    }

    // ---- #23: auto-migrate defaults off in production ----

    #[test]
    fn auto_migrate_defaults_on_in_dev_off_in_prod() {
        // Unset: on in dev, off in prod.
        assert!(resolve_auto_migrate(None, false));
        assert!(!resolve_auto_migrate(None, true));
        // Explicit off.
        assert!(!resolve_auto_migrate(Some("0"), false));
        assert!(!resolve_auto_migrate(Some("false"), false));
        assert!(!resolve_auto_migrate(Some("FALSE"), true));
        // Explicit on — honoured even in prod (deliberate opt-in escape hatch).
        assert!(resolve_auto_migrate(Some("1"), true));
        assert!(resolve_auto_migrate(Some("true"), true));
    }
}
