use super::dto::VendorProperty;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tokens::ApiPrincipal;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /api/v1/properties` — portfolio for the token's tenant. Scope: `property:read`.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[get("/api/v1/properties")]
pub async fn properties(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    principal: ApiPrincipal,
) -> ApiResult<Json<Vec<VendorProperty>>> {
    principal.require(Permission::PropertyRead)?;
    let rows = Property::find()
        .filter(entity::property::Column::TenantId.eq(principal.tenant_id))
        .order_by_asc(entity::property::Column::Name)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|p| VendorProperty {
                occupancy: format!("{}/{}", p.occupied_units, p.units),
                monthly_rent: usd(p.monthly_rent_cents),
                id: p.id,
                name: p.name,
                city: p.city,
                units: p.units,
                status: p.status,
            })
            .collect(),
    ))
}
