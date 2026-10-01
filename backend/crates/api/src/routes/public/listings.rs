use super::dto::ListingResp;
use super::search::{apply, Search};
use crate::error::ApiResult;
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use entity::prelude::Listing;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /public/listings` — public, available listings for a tenant, with
/// optional search: `q` (title, address or city), `min_rent` / `max_rent`
/// (dollars a month), `beds` and `baths` (at least), `min_sqft`,
/// `available_now`, and `sort` (`newest`, `price_asc`, `price_desc`, `beds`,
/// `sqft`).
#[rocket_okapi::openapi(tag = "Public Website")]
#[allow(clippy::too_many_arguments)]
#[get(
    "/public/listings?<q>&<min_rent>&<max_rent>&<beds>&<baths>&<min_sqft>&<available_now>&<sort>"
)]
pub async fn listings(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    q: Option<String>,
    min_rent: Option<i64>,
    max_rent: Option<i64>,
    beds: Option<i32>,
    baths: Option<i32>,
    min_sqft: Option<i32>,
    available_now: Option<bool>,
    sort: Option<String>,
) -> ApiResult<Json<Vec<ListingResp>>> {
    let rows = Listing::find()
        .filter(entity::listing::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::listing::Column::IsPublic.eq(true))
        // A leased home is never advertised, even if someone forgot to
        // unpublish it (the pipeline normally does both).
        .filter(entity::listing::Column::Status.ne("Leased"))
        .order_by_desc(entity::listing::Column::CreatedAt)
        .all(&db)
        .await?;
    let rows = rows.into_iter().map(ListingResp::from).collect();
    // A public list never answers more than this many homes.
    let mut found = apply(
        rows,
        &Search {
            q,
            min_rent,
            max_rent,
            beds,
            baths,
            min_sqft,
            available_now: available_now.unwrap_or(false),
            sort,
        },
    );
    found.truncate(PUBLIC_LIMIT);
    crate::routes::listings::photos::attach_public(&db, tenant.tenant_id, &mut found).await?;
    Ok(Json(found))
}

/// The most homes one public search answers with.
pub const PUBLIC_LIMIT: usize = 200;
