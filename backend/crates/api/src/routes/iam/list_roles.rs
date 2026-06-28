use super::dto::RoleDto;
use super::helpers::role_permissions;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::*;
use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /admin/roles?tenant_id=&scope=` — roles (system + custom).
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/roles?<tenant_id>&<scope>")]
pub async fn list_roles(
    state: &State<AppState>,
    user: AuthUser,
    tenant_id: Option<String>,
    scope: Option<String>,
) -> ApiResult<Json<Vec<RoleDto>>> {
    user.require(Permission::RoleRead)?;
    let mut q = Role::find();
    if let Some(s) = &scope {
        q = q.filter(entity::role::Column::Scope.eq(s.clone()));
    }
    if let Some(tid) = tenant_id.as_deref().and_then(|s| Uuid::parse_str(s).ok()) {
        q = q.filter(
            entity::role::Column::TenantId
                .eq(tid)
                .or(entity::role::Column::TenantId.is_null()),
        );
    }
    let roles = q
        .order_by_asc(entity::role::Column::Name)
        .all(&state.user_db)
        .await?;
    let mut out = Vec::new();
    for r in roles {
        let perms = role_permissions(&state.user_db, r.id).await?;
        out.push(RoleDto {
            id: r.id,
            scope: r.scope,
            tenant_id: r.tenant_id,
            key: r.key,
            name: r.name,
            description: r.description,
            is_system: r.is_system,
            permissions: perms,
        });
    }
    Ok(Json(out))
}
