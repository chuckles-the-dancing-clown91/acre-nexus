//! Authentication primitives: password hashing, JWT access tokens, opaque
//! refresh/secret tokens, and the `AuthUser` request guard.

use crate::error::ApiError;
use crate::rbac::{Grants, Permission};
use crate::state::AppState;
use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Passwords (Argon2id)
// ---------------------------------------------------------------------------

/// Hash a plaintext password with Argon2id and a random salt.
pub fn hash_password(plain: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash failure: {e}"))?
        .to_string();
    Ok(hash)
}

/// Verify a plaintext password against a stored Argon2 hash.
pub fn verify_password(plain: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(plain.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Opaque secrets (refresh tokens, API keys)
// ---------------------------------------------------------------------------

/// Generate a URL-safe random secret string of the given byte length.
pub fn random_secret(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    // hex keeps it copy/paste-safe and fixed-width.
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 hash of a secret, stored instead of the raw value.
pub fn hash_secret(secret: &str) -> String {
    let mut h = Sha256::new();
    h.update(secret.as_bytes());
    format!("{:x}", h.finalize())
}

// ---------------------------------------------------------------------------
// JWT access tokens
// ---------------------------------------------------------------------------

/// Claims embedded in the signed JWT access token.
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Claims {
    /// Subject — the user id.
    pub sub: Uuid,
    /// Tenant the user belongs to (absent for platform staff).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tid: Option<Uuid>,
    /// Platform-staff flag.
    pub staff: bool,
    /// Resolved permission strings.
    pub perms: Vec<String>,
    /// Expiry (unix seconds).
    pub exp: i64,
    /// Issued-at (unix seconds).
    pub iat: i64,
}

/// Sign a JWT access token for a principal.
pub fn issue_access_token(
    cfg: &crate::config::Config,
    user_id: Uuid,
    tenant_id: Option<Uuid>,
    staff: bool,
    perms: Vec<String>,
) -> anyhow::Result<String> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        sub: user_id,
        tid: tenant_id,
        staff,
        perms,
        iat: now,
        exp: now + cfg.access_ttl_secs,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(cfg.jwt_secret.as_bytes()),
    )?;
    Ok(token)
}

fn decode_access_token(cfg: &crate::config::Config, token: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}

// ---------------------------------------------------------------------------
// AuthUser request guard
// ---------------------------------------------------------------------------

/// An authenticated human principal, extracted from a `Bearer` JWT.
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub is_staff: bool,
    pub grants: Grants,
}

impl AuthUser {
    /// Assert the principal holds a permission, else `403`.
    pub fn require(&self, p: Permission) -> Result<(), ApiError> {
        self.require_key(p.as_str())
    }

    /// Assert the principal holds a string-keyed permission (built-in or custom).
    pub fn require_key(&self, key: &str) -> Result<(), ApiError> {
        if self.grants.has_key(key) {
            Ok(())
        } else {
            Err(ApiError::Forbidden(format!("missing permission: {key}")))
        }
    }
}

fn bearer(req: &Request<'_>) -> Option<String> {
    req.headers()
        .get_one("Authorization")
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let state = match req.rocket().state::<AppState>() {
            Some(s) => s,
            None => return Outcome::Error((Status::InternalServerError, ())),
        };
        let token = match bearer(req) {
            Some(t) => t,
            None => return Outcome::Error((Status::Unauthorized, ())),
        };
        match decode_access_token(&state.config, &token) {
            Some(c) => Outcome::Success(AuthUser {
                user_id: c.sub,
                tenant_id: c.tid,
                is_staff: c.staff,
                grants: Grants::from_iter(c.perms),
            }),
            None => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}
