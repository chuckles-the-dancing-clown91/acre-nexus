use super::dto::LeaseDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/leases` — list a property's leases.
#[rocket_okapi::openapi(tag = "Rentals")]
#[get("/properties/<id>/leases")]
pub async fn list_property_leases(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<LeaseDto>>> {
    user.require(Permission::LeaseRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let rows = Lease::find()
        .filter(entity::lease::Column::PropertyId.eq(pid))
        .order_by_desc(entity::lease::Column::CreatedAt)
        .all(&state.property_db)
        .await?;
    Ok(Json(rows.into_iter().map(LeaseDto::from).collect()))
}
