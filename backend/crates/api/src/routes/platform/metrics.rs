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
    // Cross-database read: tenants live in acre_user, properties in acre_property.
    // The property tables enforce row-level security on `app.tenant_id`; an
    // unclamped `.all()` is non-deterministic under connection pooling, so we
    // read each tenant's properties inside an RLS-clamped transaction and
    // aggregate the totals in application code.
    let tenants = Tenant::find().all(&state.user_db).await?;
    let mut total_properties = 0i64;
    let mut revenue = 0i64;
    for t in &tenants {
        let txn = AppState::tenant_tx(&state.property_db, t.id).await?;
        let props = Property::find().all(&txn).await?;
        total_properties += props.len() as i64;
        revenue += props.iter().map(|p| p.monthly_rent_cents).sum::<i64>();
        txn.rollback().await.ok();
    }
    Ok(Json(PlatformMetrics {
        tenant_count: tenants.len() as i64,
        active_tenants: tenants.iter().filter(|t| t.status == "active").count() as i64,
        total_properties,
        total_managed_revenue_label: usd(revenue),
    }))
}
