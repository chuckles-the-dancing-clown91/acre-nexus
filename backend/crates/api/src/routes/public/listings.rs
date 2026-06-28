use super::dto::ListingResp;
use crate::error::ApiResult;
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use entity::prelude::Listing;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /public/listings` — public, available listings for a tenant.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/listings")]
pub async fn listings(
    state: &State<AppState>,
    tenant: PublicTenant,
) -> ApiResult<Json<Vec<ListingResp>>> {
    let rows = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::listing::Column::IsPublic.eq(true))
        .order_by_desc(entity::listing::Column::CreatedAt)
        .all(&state.property_db)
        .await?;
    Ok(Json(rows.into_iter().map(ListingResp::from).collect()))
}
