//! **Vehicle** endpoints — resident vehicle profiles, attachable to an
//! application, a lease, and/or a renter user. Garage/parking amenities pull these
//! into the generated lease document.

pub mod create;
pub mod delete;
pub mod dto;
pub mod list;
pub mod portal;
pub mod update;

use crate::error::{ApiError, ApiResult};
use entity::prelude::{Application, Lease};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// Reject a `lease_id` / `application_id` that doesn't belong to `tenant_id`, so a
/// vehicle can't be attached to another tenant's lease or application (which would
/// leak it into their lease document + fee evaluation).
pub(crate) async fn assert_links_in_tenant(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    lease_id: Option<Uuid>,
    application_id: Option<Uuid>,
) -> ApiResult<()> {
    if let Some(lid) = lease_id {
        let ok = Lease::find_by_id(lid)
            .filter(entity::lease::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .is_some();
        if !ok {
            return Err(ApiError::NotFound("lease not found".into()));
        }
    }
    if let Some(aid) = application_id {
        let ok = Application::find_by_id(aid)
            .filter(entity::application::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .is_some();
        if !ok {
            return Err(ApiError::NotFound("application not found".into()));
        }
    }
    Ok(())
}
