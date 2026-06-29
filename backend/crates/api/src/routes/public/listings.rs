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
    // Clamp the RLS context to the resolved tenant so the read is deterministic
    // under connection pooling (an unclamped read can be hidden by a stale
    // app.tenant_id left on a pooled connection).
    let txn = AppState::tenant_tx(&state.property_db, tenant.tenant_id).await?;
    let rows = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::listing::Column::IsPublic.eq(true))
        .order_by_desc(entity::listing::Column::CreatedAt)
        .all(&txn)
        .await?;
    txn.rollback().await.ok();
    Ok(Json(rows.into_iter().map(ListingResp::from).collect()))
}
