use super::dto::MortgageDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Mortgage, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/mortgages` — list a property's mortgages, ordered by lien position.
#[rocket_okapi::openapi(tag = "Financing")]
#[get("/properties/<id>/mortgages")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<MortgageDto>>> {
    user.require(Permission::FinanceRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let rows = Mortgage::find()
        .filter(entity::mortgage::Column::PropertyId.eq(pid))
        .order_by_asc(entity::mortgage::Column::Position)
        .all(&state.property_db)
        .await?;
    Ok(Json(rows.into_iter().map(MortgageDto::from).collect()))
}
