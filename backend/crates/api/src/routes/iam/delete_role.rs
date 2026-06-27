use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::delete;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /admin/roles/<id>` — delete a custom role (system roles are protected).
#[rocket_okapi::openapi(tag = "IAM")]
#[delete("/admin/roles/<id>")]
pub async fn delete_role(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::RoleManage)?;
    let rid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid role id".into()))?;
    let role = Role::find_by_id(rid)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("role not found".into()))?;
    if role.is_system {
        return Err(ApiError::Forbidden("system roles cannot be deleted".into()));
    }
    RolePermission::delete_many()
        .filter(entity::role_permission::Column::RoleId.eq(rid))
        .exec(&state.db)
        .await?;
    UserRole::delete_many()
        .filter(entity::user_role::Column::RoleId.eq(rid))
        .exec(&state.db)
        .await?;
    Role::delete_by_id(rid).exec(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::ROLE_DELETE,
        Some("role"),
        Some(rid.to_string()),
        role.tenant_id,
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
