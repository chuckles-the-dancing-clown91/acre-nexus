use super::dto::VendorListing;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tokens::ApiPrincipal;
use entity::prelude::Listing;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /api/v1/listings` — listings for the token's tenant. Scope: `listing:read`.
#[rocket_okapi::openapi(tag = "Vendor API")]
#[get("/api/v1/listings")]
pub async fn listings(
    state: &State<AppState>,
    principal: ApiPrincipal,
) -> ApiResult<Json<Vec<VendorListing>>> {
    principal.require(Permission::ListingRead)?;
    let rows = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(principal.tenant_id))
        .order_by_desc(entity::listing::Column::CreatedAt)
        .all(&state.property_db)
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
