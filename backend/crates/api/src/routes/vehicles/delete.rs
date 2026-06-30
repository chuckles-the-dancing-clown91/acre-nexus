//! `DELETE /vehicles/<id>` — remove a vehicle profile.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Vehicle;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /vehicles/<id>` — delete a vehicle.
#[rocket_okapi::openapi(tag = "Vehicles")]
#[delete("/vehicles/<id>")]
pub async fn delete(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::VehicleManage)?;
    let vid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let v = Vehicle::find_by_id(vid)
        .filter(entity::vehicle::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("vehicle not found".into()))?;
    v.delete(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::VEHICLE_DELETE,
        Some("vehicle"),
        Some(vid.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
