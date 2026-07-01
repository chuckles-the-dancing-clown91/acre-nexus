use super::dto::ProfileTypeDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use rocket::get;
use rocket::serde::json::Json;
use rocket::State;
use sea_orm::{EntityTrait, QueryOrder};

/// `GET /admin/profile-types` — the persona catalog.
#[rocket_okapi::openapi(tag = "IAM")]
#[get("/admin/profile-types")]
pub async fn profile_types(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
) -> ApiResult<Json<Vec<ProfileTypeDto>>> {
    user.require(Permission::MemberRead)?;
    let rows = entity::profile_type::Entity::find()
        .order_by_asc(entity::profile_type::Column::Scope)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|p| ProfileTypeDto {
                key: p.key,
                scope: p.scope,
                label: p.label,
                description: p.description,
                default_role: p.default_role,
            })
            .collect(),
    ))
}
