use super::dto::PermissionDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{EntityTrait, QueryOrder};

/// `GET /admin/permissions` — the permission catalog (for the role editor).
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/permissions")]
pub async fn permissions(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
) -> ApiResult<Json<Vec<PermissionDto>>> {
    user.require(Permission::RoleRead)?;
    let rows = entity::permission::Entity::find()
        .order_by_asc(entity::permission::Column::Category)
        .order_by_asc(entity::permission::Column::Key)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|p| PermissionDto {
                key: p.key,
                category: p.category,
                label: p.label,
                description: p.description,
                scope: p.scope,
            })
            .collect(),
    ))
}
