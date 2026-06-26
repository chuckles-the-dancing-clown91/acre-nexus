//! Versioned **vendor API** (`/api/v1`). Authenticated with scoped API tokens
//! (not JWTs), this is the surface sold to third-party vendors so Acre services
//! can be leveraged à la carte. Each endpoint requires a specific token scope.

use crate::dto::usd;
use crate::error::ApiResult;
use crate::state::AppState;
use crate::tokens::ApiPrincipal;
use crate::rbac::Permission;
use entity::prelude::{Listing, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct VendorListing {
    pub id: Uuid,
    pub title: String,
    pub city: String,
    pub beds: i32,
    pub baths: i32,
    pub rent: String,
    pub status: String,
}

/// `GET /api/v1/listings` — listings for the token's tenant. Scope: `listing:read`.
#[get("/api/v1/listings")]
pub async fn listings(
    state: &State<AppState>,
    principal: ApiPrincipal,
) -> ApiResult<Json<Vec<VendorListing>>> {
    principal.require(Permission::ListingRead)?;
    let rows = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(principal.tenant_id))
        .order_by_desc(entity::listing::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|l| VendorListing {
                id: l.id,
                title: l.title,
                city: l.city,
                beds: l.beds,
                baths: l.baths,
                rent: usd(l.rent_cents),
                status: l.status,
            })
            .collect(),
    ))
}

#[derive(Serialize)]
pub struct VendorProperty {
    pub id: Uuid,
    pub name: String,
    pub city: String,
    pub units: i32,
    pub occupancy: String,
    pub monthly_rent: String,
    pub status: String,
}

/// `GET /api/v1/properties` — portfolio for the token's tenant. Scope: `property:read`.
#[get("/api/v1/properties")]
pub async fn properties(
    state: &State<AppState>,
    principal: ApiPrincipal,
) -> ApiResult<Json<Vec<VendorProperty>>> {
    principal.require(Permission::PropertyRead)?;
    let rows = Property::find()
        .filter(entity::property::Column::TenantId.eq(principal.tenant_id))
        .order_by_asc(entity::property::Column::Name)
        .all(&state.db)
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
