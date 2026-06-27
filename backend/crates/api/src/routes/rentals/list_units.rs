use super::dto::UnitDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Property, Unit};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/units` — list a property's rentable units.
#[rocket_okapi::openapi(tag = "Rentals")]
#[get("/properties/<id>/units")]
pub async fn list_units(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<UnitDto>>> {
    user.require(Permission::LeaseRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let rows = Unit::find()
        .filter(entity::unit::Column::PropertyId.eq(pid))
        .order_by_asc(entity::unit::Column::UnitNumber)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(UnitDto::from).collect()))
}
