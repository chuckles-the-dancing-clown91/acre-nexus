//! Platform (Acre HQ) admin endpoints — **staff only**, cross-tenant. These are
//! the SaaS-vendor's own console: client companies and platform metrics. Client
//! users can never reach these (gated by the `platform:admin` permission).

use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use entity::prelude::{Property, Tenant};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct TenantSummary {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub plan: String,
    pub status: String,
    pub custom_domain: Option<String>,
    pub property_count: i64,
    pub managed_revenue_label: String,
}

/// `GET /platform/tenants` — every client company on the platform.
#[rocket_okapi::openapi(tag = "Platform Admin")]
#[get("/platform/tenants")]
pub async fn tenants(
    state: &State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<TenantSummary>>> {
    user.require(Permission::PlatformAdmin)?;
    let all = Tenant::find()
        .order_by_asc(entity::tenant::Column::Name)
        .all(&state.db)
        .await?;

    let mut out = Vec::new();
    for t in all {
        let props = Property::find()
            .filter(entity::property::Column::TenantId.eq(t.id))
            .all(&state.db)
            .await?;
        let revenue: i64 = props.iter().map(|p| p.monthly_rent_cents).sum();
        out.push(TenantSummary {
            id: t.id,
            slug: t.slug,
            name: t.name,
            plan: t.plan,
            status: t.status,
            custom_domain: t.custom_domain,
            property_count: props.len() as i64,
            managed_revenue_label: usd(revenue),
        });
    }
    Ok(Json(out))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PlatformMetrics {
    pub tenant_count: i64,
    pub active_tenants: i64,
    pub total_properties: i64,
    pub total_managed_revenue_label: String,
}

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
