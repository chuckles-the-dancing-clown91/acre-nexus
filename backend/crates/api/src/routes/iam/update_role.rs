use super::dto::{RoleDto, UpdateRoleReq};
use super::helpers::{replace_role_permissions, role_permissions, validate_permissions};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::patch;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;

/// `PATCH /admin/roles/<id>` — rename / re-describe and/or replace permissions.
#[rocket_okapi::openapi(tag = "IAM")]
#[patch("/admin/roles/<id>", data = "<body>")]
pub async fn update_role(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    id: &str,
    body: Json<UpdateRoleReq>,
) -> ApiResult<Json<RoleDto>> {
    user.require(Permission::RoleManage)?;
    let rid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid role id".into()))?;
    let role = Role::find_by_id(rid)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("role not found".into()))?;
    let body = body.into_inner();

    let mut am: entity::role::ActiveModel = role.clone().into();
    if let Some(name) = body.name.clone() {
        am.name = Set(name);
    }
    if let Some(desc) = body.description.clone() {
        am.description = Set(desc);
    }
    am.update(&db).await?;

    if let Some(perms) = &body.permissions {
        validate_permissions(perms)?;
        replace_role_permissions(&db, rid, perms).await?;
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ROLE_UPDATE,
        Some("role"),
        Some(rid.to_string()),
        role.tenant_id,
        None,
    )
    .await;
    let updated = Role::find_by_id(rid).one(&db).await?.unwrap();
    let perms = role_permissions(&db, rid).await?;
    Ok(Json(RoleDto {
        id: updated.id,
        scope: updated.scope,
        tenant_id: updated.tenant_id,
        key: updated.key,
        name: updated.name,
        description: updated.description,
        is_system: updated.is_system,
        permissions: perms,
    }))
}
