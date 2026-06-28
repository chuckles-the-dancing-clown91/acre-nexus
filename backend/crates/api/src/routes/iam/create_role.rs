use super::dto::{CreateRoleReq, RoleDto};
use super::helpers::{replace_role_permissions, validate_permissions};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::{self, Permission};
use crate::state::AppState;
use rocket::post;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// `POST /admin/roles` — create a custom role with a permission set.
#[rocket_okapi::openapi(tag = "IAM")]
#[post("/admin/roles", data = "<body>")]
pub async fn create_role(
    state: &State<AppState>,
    user: AuthUser,
    body: Json<CreateRoleReq>,
) -> ApiResult<Json<RoleDto>> {
    user.require(Permission::RoleManage)?;
    let body = body.into_inner();
    if body.scope != rbac::SCOPE_PLATFORM && body.scope != rbac::SCOPE_TENANT {
        return Err(ApiError::BadRequest(
            "scope must be 'platform' or 'tenant'".into(),
        ));
    }
    validate_permissions(&body.permissions)?;
    let id = Uuid::new_v4();
    entity::role::ActiveModel {
        id: Set(id),
        tenant_id: Set(body.tenant_id),
        scope: Set(body.scope.clone()),
        key: Set(body.key.clone()),
        name: Set(body.name.clone()),
        description: Set(body.description.clone()),
        is_system: Set(false),
    }
    .insert(&state.user_db)
    .await?;
    replace_role_permissions(&state.user_db, id, &body.permissions).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::ROLE_CREATE,
        Some("role"),
        Some(id.to_string()),
        body.tenant_id,
        Some(serde_json::json!({ "key": body.key, "permissions": body.permissions.len() })),
    )
    .await;
    Ok(Json(RoleDto {
        id,
        scope: body.scope,
        tenant_id: body.tenant_id,
        key: body.key,
        name: body.name,
        description: body.description,
        is_system: false,
        permissions: body.permissions,
    }))
}
