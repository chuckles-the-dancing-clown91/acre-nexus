//! Runtime configuration, loaded from environment variables (and `.env`).

use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    /// Postgres connection string.
    pub database_url: String,
    /// HMAC secret for signing JWT access tokens. **Override in production.**
    pub jwt_secret: String,
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
        Config {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://localhost:5432/acre".into()),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "dev-insecure-change-me".into()),
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
