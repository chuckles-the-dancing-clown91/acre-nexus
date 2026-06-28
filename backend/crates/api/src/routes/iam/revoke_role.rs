use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::delete;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::EntityTrait;

/// `DELETE /admin/user-roles/<id>` — revoke a role assignment.
#[rocket_okapi::openapi(tag = "IAM")]
#[delete("/admin/user-roles/<id>")]
pub async fn revoke_role(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::RoleManage)?;
    let urid: i64 = id
        .parse()
        .map_err(|_| ApiError::BadRequest("invalid assignment id".into()))?;
    UserRole::delete_by_id(urid).exec(&state.user_db).await?;
    Ok(Json(serde_json::json!({ "revoked": true })))
}
