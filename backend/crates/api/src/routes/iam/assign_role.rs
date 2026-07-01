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
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
    body: Json<AssignRoleReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::RoleManage)?;
    let uid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid user id".into()))?;
    let body = body.into_inner();
    if Role::find_by_id(body.role_id).one(&db).await?.is_none() {
        return Err(ApiError::NotFound("role not found".into()));
    }

    // Resolve + validate the coverage scope (defaults to platform/tenant by context).
    let scope = body.scope.clone().unwrap_or_else(|| {
        if body.tenant_id.is_some() {
            crate::rbac::scope::SCOPE_TENANT.into()
        } else {
            crate::rbac::scope::SCOPE_PLATFORM.into()
        }
    });
    if !crate::rbac::scope::is_valid_scope(&scope) {
        return Err(ApiError::BadRequest(format!("invalid scope: {scope}")));
    }
    // Narrower-than-tenant scopes must name the resource they cover.
    if crate::rbac::scope::is_resource_scope(&scope) && body.scope_ref_id.is_none() {
        return Err(ApiError::BadRequest(format!(
            "scope '{scope}' requires scope_ref_id"
        )));
    }

    entity::user_role::ActiveModel {
        id: sea_orm::ActiveValue::NotSet,
        user_id: Set(uid),
        role_id: Set(body.role_id),
        tenant_id: Set(body.tenant_id),
        scope: Set(scope.clone()),
        scope_ref_id: Set(body.scope_ref_id),
    }
    .insert(&db)
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ROLE_ASSIGN,
        Some("user"),
        Some(uid.to_string()),
        body.tenant_id,
        Some(serde_json::json!({
            "role_id": body.role_id,
            "scope": scope,
            "scope_ref_id": body.scope_ref_id,
        })),
    )
    .await;
    Ok(Json(serde_json::json!({ "assigned": true })))
}
