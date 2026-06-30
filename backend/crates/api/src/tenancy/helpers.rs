use crate::error::ApiError;
use crate::state::AppState;
use entity::prelude::{Domain, Tenant};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// A host resolved via the `domain` table: which tenant it routes to and which
/// app surface (`admin` / `owner` / `renter`) it serves.
pub(crate) struct ResolvedHost {
    pub tenant_id: Uuid,
    pub audience: String,
}

/// Resolve an inbound `Host` header to a tenant + audience via the `domain` table
/// (§7.2). Only verified domains route; an unverified custom domain returns
/// `None` so the caller can fall back to the marketing/landing surface.
pub(crate) async fn resolve_host(state: &AppState, hostname: &str) -> Option<ResolvedHost> {
    let host = hostname
        .split(':')
        .next()
        .unwrap_or(hostname)
        .to_lowercase();
    Domain::find()
        .filter(entity::domain::Column::Hostname.eq(host))
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .filter(|d| d.verified_at.is_some())
        .map(|d| ResolvedHost {
            tenant_id: d.tenant_id,
            audience: d.audience,
        })
}

pub(crate) async fn resolve_tenant_ref(state: &AppState, reference: &str) -> Option<Uuid> {
    // Accept either a raw uuid or a slug.
    if let Ok(id) = Uuid::parse_str(reference) {
        return Some(id);
    }
    Tenant::find()
        .filter(entity::tenant::Column::Slug.eq(reference))
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .map(|t| t.id)
}

/// Helper for handlers that need to turn a guard failure into a JSON error.
#[allow(dead_code)]
pub fn tenant_required() -> ApiError {
    ApiError::BadRequest("tenant context required — pass X-Tenant header or ?tenant=<slug>".into())
}
