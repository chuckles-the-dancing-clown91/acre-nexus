//! `DELETE /fees/<id>` — remove a fee-schedule entry.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::FeeSchedule;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /fees/<id>` — delete a fee-schedule entry.
#[rocket_okapi::openapi(tag = "Fee Schedule")]
#[delete("/fees/<id>")]
pub async fn delete(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::FeeManage)?;
    let fid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let fee = FeeSchedule::find_by_id(fid)
        .filter(entity::fee_schedule::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("fee not found".into()))?;
    fee.delete(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::FEE_SCHEDULE_DELETE,
        Some("fee_schedule"),
        Some(fid.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
