use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

/// Decode + verify a JWT access token into its [`Claims`]. Returns `None` for an
/// invalid/expired signature. Used by the `AuthUser` guard and the audit fairing.
pub(crate) fn decode_access_token(cfg: &crate::config::Config, token: &str) -> Option<Claims> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(cfg.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .ok()
    .map(|d| d.claims)
}
