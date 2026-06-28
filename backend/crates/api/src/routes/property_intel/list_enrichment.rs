//! `GET /properties/<id>/enrichment` — recent enrichment runs for a property
//! (the observable trail of automated fetches, newest first).

use super::dto::EnrichmentRunDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{EnrichmentRun, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use uuid::Uuid;

/// `GET /properties/<id>/enrichment?limit=` — recent enrichment runs.
#[rocket_okapi::openapi(tag = "Property Intelligence")]
#[get("/properties/<id>/enrichment?<limit>")]
pub async fn list_enrichment(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    limit: Option<u64>,
) -> ApiResult<Json<Vec<EnrichmentRunDto>>> {
    user.require(Permission::PropertyRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let rows = EnrichmentRun::find()
        .filter(entity::enrichment_run::Column::PropertyId.eq(pid))
        .order_by_desc(entity::enrichment_run::Column::CreatedAt)
        .limit(limit.unwrap_or(50).min(200))
        .all(&state.property_db)
        .await?;
    Ok(Json(rows.into_iter().map(EnrichmentRunDto::from).collect()))
}
