//! Address suggestions and property photos (see [`crate::geo`]).

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::geo::{self, PhotoResult, Place};
use crate::rbac::Permission;
use crate::routes::team::parse_id;
use crate::tenancy::TenantScope;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{get, post};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;

/// `GET /geo/suggest?q&limit` — address suggestions as you type.
#[rocket_okapi::openapi(tag = "Geo")]
#[get("/geo/suggest?<q>&<limit>")]
pub async fn suggest(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    q: String,
    limit: Option<usize>,
) -> ApiResult<Json<Vec<Place>>> {
    user.require(Permission::PropertyRead)?;
    Ok(Json(
        geo::suggest(&db, scope.tenant_id, &q, limit.unwrap_or(6)).await?,
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct GeoStatus {
    /// `google` | `photon`
    pub suggestions: String,
    /// `live` (Street View / satellite) | `placeholder`
    pub photos: String,
    pub has_key: bool,
    pub live: bool,
}

/// `GET /geo/status` — which map provider is in use.
#[rocket_okapi::openapi(tag = "Geo")]
#[get("/geo/status")]
pub async fn status(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<GeoStatus>> {
    user.require(Permission::PropertyRead)?;
    let has_key = geo::maps_key(&db, scope.tenant_id).await.is_some();
    let live = geo::google_live(&db, scope.tenant_id).await;
    Ok(Json(GeoStatus {
        suggestions: if live { "google" } else { "photon" }.into(),
        photos: if live { "live" } else { "placeholder" }.into(),
        has_key,
        live,
    }))
}

/// `POST /properties/<id>/photo` — fetch (or refresh) the street photo now.
#[rocket_okapi::openapi(tag = "Geo")]
#[post("/properties/<id>/photo")]
pub async fn fetch_photo(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PhotoResult>> {
    user.require(Permission::PropertyWrite)?;
    let p = Property::find_by_id(parse_id(id, "property")?)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    Ok(Json(geo::fetch_photo(&db, scope.tenant_id, p).await?))
}
