//! Shared internals for the LLC onboarding handlers.

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use entity::prelude::Llc;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Parse a path-segment UUID, mapping failure to a 400.
pub fn parse_uuid(s: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(s).map_err(|_| ApiError::BadRequest("invalid id".into()))
}

/// Load an LLC, enforcing tenant ownership (404 if it isn't this tenant's).
pub async fn require_llc(
    state: &AppState,
    tenant_id: Uuid,
    llc_id: Uuid,
) -> ApiResult<entity::llc::Model> {
    Llc::find_by_id(llc_id)
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("llc not found".into()))
}

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Reduce a client-supplied extension to a safe object-key suffix (lowercase
/// alphanumerics, capped) so it can't smuggle path/control characters into the
/// storage key. Falls back to `bin`.
pub fn sanitize_ext(ext: &str) -> String {
    let cleaned: String = ext
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_ascii_lowercase();
    if cleaned.is_empty() {
        "bin".into()
    } else {
        cleaned
    }
}
