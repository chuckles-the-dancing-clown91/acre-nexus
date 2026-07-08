//! Acquisition **deal pipeline** routes (roadmap Phase 7, issues #41/#42),
//! contributed by the `flips` module and self-gated on its per-tenant
//! enablement. A deal moves `prospecting → offer → under_contract → closing →
//! owned`, carries its underwriting assumptions, and converts into a property.

pub mod advance;
pub mod checklist;
pub mod convert;
pub mod create;
pub mod dto;
pub mod get;
pub mod list;
pub mod underwrite;
pub mod update;

use crate::error::{ApiError, ApiResult};
use entity::prelude::Deal;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// Load a deal by id, scoped to the active tenant (404 if it isn't there).
pub async fn load_deal(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::deal::Model> {
    let did = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid deal id".into()))?;
    Deal::find_by_id(did)
        .filter(entity::deal::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("deal not found".into()))
}
