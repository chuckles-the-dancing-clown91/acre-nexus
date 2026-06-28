use crate::error::ApiError;
use crate::state::AppState;
use entity::prelude::Tenant;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

pub(crate) async fn resolve_tenant_ref(state: &AppState, reference: &str) -> Option<Uuid> {
    // Accept either a raw uuid or a slug.
    if let Ok(id) = Uuid::parse_str(reference) {
        return Some(id);
    }
    Tenant::find()
        .filter(entity::tenant::Column::Slug.eq(reference))
        .one(&state.user_db)
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
