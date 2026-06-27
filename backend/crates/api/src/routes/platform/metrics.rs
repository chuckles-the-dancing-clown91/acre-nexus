use super::dto::PlatformMetrics;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::{Property, Tenant};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::EntityTrait;

/// `GET /platform/metrics` — top-line platform metrics (MRR-style overview).
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[get("/platform/metrics")]
pub async fn metrics(state: &State<AppState>, user: AuthUser) -> ApiResult<Json<PlatformMetrics>> {
    user.require(Permission::PlatformAdmin)?;
    let tenants = Tenant::find().all(&state.db).await?;
    let props = Property::find().all(&state.db).await?;
    let revenue: i64 = props.iter().map(|p| p.monthly_rent_cents).sum();
    Ok(Json(PlatformMetrics {
        tenant_count: tenants.len() as i64,
        active_tenants: tenants.iter().filter(|t| t.status == "active").count() as i64,
        total_properties: props.len() as i64,
        total_managed_revenue_label: usd(revenue),
    }))
}
