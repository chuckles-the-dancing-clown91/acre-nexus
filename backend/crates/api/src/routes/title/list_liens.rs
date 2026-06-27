use super::dto::LienDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lien, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/liens` — list liens recorded against a property, by position.
#[rocket_okapi::openapi(tag = "Title")]
#[get("/properties/<id>/liens")]
pub async fn list_liens(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<LienDto>>> {
    user.require(Permission::TitleRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let rows = Lien::find()
        .filter(entity::lien::Column::PropertyId.eq(pid))
        .order_by_asc(entity::lien::Column::Position)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(LienDto::from).collect()))
}
