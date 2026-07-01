use super::dto::OwnershipDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Ownership, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `GET /properties/<id>/ownership` — list a property's ownership records.
#[rocket_okapi::openapi(tag = "Title")]
#[get("/properties/<id>/ownership")]
pub async fn list_ownership(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<OwnershipDto>>> {
    user.require(Permission::TitleRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let rows = Ownership::find()
        .filter(entity::ownership::Column::PropertyId.eq(pid))
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(OwnershipDto::from).collect()))
}
