use super::dto::ListingResp;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use entity::prelude::Listing;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `GET /public/listings/<id>` — a single public listing.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/listings/<id>")]
pub async fn listing_detail(
    state: &State<AppState>,
    tenant: PublicTenant,
    id: &str,
) -> ApiResult<Json<ListingResp>> {
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    // Clamp the RLS context to the resolved tenant (deterministic under pooling).
    let txn = AppState::tenant_tx(&state.property_db, tenant.tenant_id).await?;
    let found = Listing::find_by_id(lid)
        .filter(entity::listing::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::listing::Column::IsPublic.eq(true))
        .one(&txn)
        .await?;
    txn.rollback().await.ok();
    let l = found.ok_or_else(|| ApiError::NotFound("listing not found".into()))?;
    Ok(Json(ListingResp::from(l)))
}
