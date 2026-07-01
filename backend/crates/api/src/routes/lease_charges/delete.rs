//! `DELETE /lease-charges/<id>` — remove a line item from a lease.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::LeaseCharge;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /lease-charges/<id>` — delete a charge.
#[rocket_okapi::openapi(tag = "Lease Charges")]
#[delete("/lease-charges/<id>")]
pub async fn delete(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::LeaseManage)?;
    let cid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let charge = LeaseCharge::find_by_id(cid)
        .filter(entity::lease_charge::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("charge not found".into()))?;
    let lease_id = charge.lease_id;
    charge.delete(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEASE_CHARGE_REMOVE,
        Some("lease_charge"),
        Some(cid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": lease_id })),
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
