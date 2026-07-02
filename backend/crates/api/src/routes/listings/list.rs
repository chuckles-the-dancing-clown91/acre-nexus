//! `GET /listings` — the console view of every listing (public and not),
//! optionally filtered by property or status.

use super::dto::ConsoleListingResp;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Listing;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /listings?property_id&status` — all of this workspace's listings.
#[rocket_okapi::openapi(tag = "Listings")]
#[get("/listings?<property_id>&<status>")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    property_id: Option<String>,
    status: Option<String>,
) -> ApiResult<Json<Vec<ConsoleListingResp>>> {
    user.require(Permission::ListingRead)?;
    let mut q = Listing::find().filter(entity::listing::Column::TenantId.eq(scope.tenant_id));
    if let Some(pid) = property_id {
        let pid = Uuid::parse_str(&pid)
            .map_err(|_| ApiError::BadRequest("invalid property_id".into()))?;
        q = q.filter(entity::listing::Column::PropertyId.eq(pid));
    }
    if let Some(s) = status {
        q = q.filter(entity::listing::Column::Status.eq(s));
    }
    let rows = q
        .order_by_desc(entity::listing::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter().map(ConsoleListingResp::from).collect(),
    ))
}
