use super::dto::AssignRoleReq;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::post;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;

/// `POST /admin/users/<id>/roles` — grant a role to a user (optionally tenant-scoped).
#[rocket_okapi::openapi(tag = "IAM")]
#[post("/admin/users/<id>/roles", data = "<body>")]
pub async fn assign_role(
    state: &State<AppState>,
    user: AuthUser,
    id: &str,
    body: Json<AssignRoleReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::RoleManage)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    let body = body.into_inner();
    if Role::find_by_id(body.role_id)
        .one(&state.user_db)
        .await?
        .is_none()
    {
        return Err(ApiError::NotFound("role not found".into()));
    }
    entity::user_role::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        user_id: Set(uid),
        role_id: Set(body.role_id),
        tenant_id: Set(body.tenant_id),
    }
    .insert(&state.user_db)
    .await?;
    Ok(Json(serde_json::json!({ "assigned": true })))
}
