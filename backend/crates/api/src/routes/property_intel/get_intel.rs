//! `GET /properties/<id>/intel` — the aggregated property-intelligence payload
//! (parcel/county detail, valuation history, tax history, schools, utilities).

use super::dto::{IntelResp, PropertyDetailDto, SchoolDto, TaxDto, UtilityDto, ValuationDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::*;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/intel` — all enriched data for a property.
#[rocket_okapi::openapi(tag = "Property Intelligence")]
#[get("/properties/<id>/intel")]
pub async fn get_intel(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<IntelResp>> {
    user.require(Permission::PropertyRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    // Confirm the property exists in the active tenant before exposing data.
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let detail = PropertyDetail::find_by_id(pid)
        .one(&state.db)
        .await?
        .map(PropertyDetailDto::from);

    let valuations = PropertyValuation::find()
        .filter(entity::property_valuation::Column::PropertyId.eq(pid))
        .order_by_desc(entity::property_valuation::Column::CreatedAt)
        .all(&state.db)
        .await?
        .into_iter()
        .map(ValuationDto::from)
        .collect();

    let taxes = PropertyTax::find()
        .filter(entity::property_tax::Column::PropertyId.eq(pid))
        .order_by_desc(entity::property_tax::Column::TaxYear)
        .all(&state.db)
        .await?
        .into_iter()
        .map(TaxDto::from)
        .collect();

    let schools = PropertySchool::find()
        .filter(entity::property_school::Column::PropertyId.eq(pid))
        .all(&state.db)
        .await?
        .into_iter()
        .map(SchoolDto::from)
        .collect();

    let utilities = PropertyUtility::find()
        .filter(entity::property_utility::Column::PropertyId.eq(pid))
        .all(&state.db)
        .await?
        .into_iter()
        .map(UtilityDto::from)
        .collect();

    Ok(Json(IntelResp {
        detail,
        valuations,
        taxes,
        schools,
        utilities,
    }))
}
