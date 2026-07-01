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
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::RoleManage)?;
    let urid: i64 = id
        .parse()
        .map_err(|_| ApiError::BadRequest("invalid assignment id".into()))?;
    // Fetch first so the audit entry captures which grant was revoked.
    let ur = UserRole::find_by_id(urid)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("role assignment not found".into()))?;
    UserRole::delete_by_id(urid).exec(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ROLE_REVOKE,
        Some("user"),
        Some(ur.user_id.to_string()),
        ur.tenant_id,
        Some(serde_json::json!({
            "role_id": ur.role_id,
            "scope": ur.scope,
            "scope_ref_id": ur.scope_ref_id,
        })),
    )
    .await;

    Ok(Json(serde_json::json!({ "revoked": true })))
}
