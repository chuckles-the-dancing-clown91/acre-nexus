use super::dto::{Kpi, PortfolioSummary};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// `GET /portfolio/summary` — top-line KPIs for the active tenant.
#[rocket_okapi::openapi(tag = "Portfolio")]
#[get("/portfolio/summary")]
pub async fn summary(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<PortfolioSummary>> {
    user.require(Permission::PropertyRead)?;
    let props = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
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
