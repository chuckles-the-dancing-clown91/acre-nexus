//! **Investor / syndication** routes (issue #13): capital **commitments**,
//! **capital calls** (split pro-rata by committed capital), and **distributions**
//! run through the three-tier waterfall (see [`crate::syndication`]). All hang off
//! a legal entity (LLC); gated by `investor:read` / `investor:manage` and the
//! per-tenant `syndication` module toggle.

pub mod capital_calls;
pub mod commitments;
pub mod distributions;
pub mod dto;

use crate::error::{ApiError, ApiResult};
use entity::prelude::{Llc, Owner};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;
use uuid::Uuid;

/// Load a legal entity (LLC) scoped to the tenant, or `404`.
pub(crate) async fn load_entity(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    entity_id: &str,
) -> ApiResult<entity::llc::Model> {
    let eid =
        Uuid::parse_str(entity_id).map_err(|_| ApiError::BadRequest("invalid entity id".into()))?;
    Llc::find_by_id(eid)
        .one(db)
        .await?
        .filter(|l| l.tenant_id == tenant_id)
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))
}

/// An `owner_id -> name` map for the tenant's owners (for display).
pub(crate) async fn owner_names(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
) -> ApiResult<HashMap<Uuid, String>> {
    Ok(Owner::find()
        .filter(entity::owner::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|o| (o.id, o.name))
        .collect())
}
