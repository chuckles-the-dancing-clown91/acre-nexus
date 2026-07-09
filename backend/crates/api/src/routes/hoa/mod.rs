//! **HOA / association management** routes (issue #13): associations, members,
//! dues **assessments**, CC&R **violations**, and architectural (**ARC**)
//! requests. Gated by `hoa:read` / `hoa:manage` and the per-tenant `hoa` module
//! toggle.

pub mod arc;
pub mod assessments;
pub mod associations;
pub mod dto;
pub mod members;
pub mod violations;

use crate::error::{ApiError, ApiResult};
use entity::prelude::{HoaAssociation, HoaMember};
use sea_orm::EntityTrait;
use uuid::Uuid;

pub(crate) const MODULE_KEY: &str = "hoa";

fn parse_id(id: &str, what: &str) -> ApiResult<Uuid> {
    Uuid::parse_str(id).map_err(|_| ApiError::BadRequest(format!("invalid {what} id")))
}

/// Load an association scoped to the tenant, or `404`.
pub(crate) async fn load_association(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::hoa_association::Model> {
    HoaAssociation::find_by_id(parse_id(id, "association")?)
        .one(db)
        .await?
        .filter(|a| a.tenant_id == tenant_id)
        .ok_or_else(|| ApiError::NotFound("association not found".into()))
}

/// Load a member and confirm it belongs to `association_id` in this tenant.
pub(crate) async fn load_member(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    association_id: Uuid,
    member_id: Uuid,
) -> ApiResult<entity::hoa_member::Model> {
    HoaMember::find_by_id(member_id)
        .one(db)
        .await?
        .filter(|m| m.tenant_id == tenant_id && m.association_id == association_id)
        .ok_or_else(|| ApiError::NotFound("member not found in this association".into()))
}
