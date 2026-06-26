//! Portfolio dashboard endpoints: KPI summary and LLC-grouped portfolio.

use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Llc, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct Kpi {
    pub label: String,
    pub value: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PortfolioSummary {
    pub properties: i64,
    pub units: i64,
    pub occupied_units: i64,
    pub occupancy_pct: i64,
    pub monthly_revenue_cents: i64,
    pub kpis: Vec<Kpi>,
}

/// `GET /portfolio/summary` — top-line KPIs for the active tenant.
#[rocket_okapi::openapi(tag = "Portfolio")]
#[get("/portfolio/summary")]
pub async fn summary(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<PortfolioSummary>> {
    user.require(Permission::PropertyRead)?;
    let props = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&state.db)
        .await?;

    let count = props.len() as i64;
    let units: i64 = props.iter().map(|p| p.units as i64).sum();
    let occ: i64 = props.iter().map(|p| p.occupied_units as i64).sum();
    let revenue: i64 = props.iter().map(|p| p.monthly_rent_cents).sum();
    let occ_pct = if units > 0 { occ * 100 / units } else { 0 };

    let kpis = vec![
        Kpi {
            label: "Monthly revenue".into(),
            value: usd(revenue),
        },
        Kpi {
            label: "Properties".into(),
            value: count.to_string(),
        },
        Kpi {
            label: "Units".into(),
            value: units.to_string(),
        },
        Kpi {
            label: "Occupancy".into(),
            value: format!("{occ_pct}%"),
        },
    ];

    Ok(Json(PortfolioSummary {
        properties: count,
        units,
        occupied_units: occ,
        occupancy_pct: occ_pct,
        monthly_revenue_cents: revenue,
        kpis,
    }))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LlcGroup {
    pub id: Uuid,
    pub name: String,
    pub ein: String,
    pub state: String,
    pub property_count: usize,
    pub units: i64,
    pub monthly_rent_cents: i64,
    pub monthly_rent_label: String,
    pub properties: Vec<super::properties::PropertyResp>,
}

/// `GET /portfolio/llcs` — properties grouped by holding entity.
#[rocket_okapi::openapi(tag = "Portfolio")]
#[get("/portfolio/llcs")]
pub async fn llc_groups(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<LlcGroup>>> {
    user.require(Permission::PropertyRead)?;
    let llcs = Llc::find()
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::llc::Column::Name)
        .all(&state.db)
        .await?;
    let props = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&state.db)
        .await?;

    let groups = llcs
        .into_iter()
        .map(|l| {
            let in_llc: Vec<_> = props
                .iter()
                .filter(|p| p.llc_id == Some(l.id))
                .cloned()
                .collect();
            let units: i64 = in_llc.iter().map(|p| p.units as i64).sum();
            let rent: i64 = in_llc.iter().map(|p| p.monthly_rent_cents).sum();
            LlcGroup {
                id: l.id,
                name: l.name,
                ein: l.ein,
                state: l.state,
                property_count: in_llc.len(),
                units,
                monthly_rent_cents: rent,
                monthly_rent_label: usd(rent),
                properties: in_llc
                    .into_iter()
                    .map(super::properties::PropertyResp::from)
                    .collect(),
            }
        })
        .collect();

    Ok(Json(groups))
}
