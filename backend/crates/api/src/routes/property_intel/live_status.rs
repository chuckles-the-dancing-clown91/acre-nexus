//! `GET /property-data/live` — which property-data sources are live for this
//! workspace and whether their keys are set, for the Settings page.

use crate::auth::AuthUser;
use crate::enrichment::live::{self, LiveStatus};
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use rocket::get;
use rocket::serde::json::Json;

/// `GET /property-data/live`.
#[rocket_okapi::openapi(tag = "Property Intelligence")]
#[get("/property-data/live")]
pub async fn live_status(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<LiveStatus>> {
    user.require(Permission::PropertyRead)?;
    Ok(Json(live::status(&db, scope.tenant_id).await))
}
