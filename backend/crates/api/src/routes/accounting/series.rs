use super::dto::FinanceSeriesResp;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{get, State};

/// `GET /finance/series?months=12` — the dashboard time series: rent
/// due/collected and NOI computed live from the payments table + ledger,
/// occupancy/delinquency/portfolio value from the monthly snapshot history
/// (current month always live).
#[rocket_okapi::openapi(tag = "Accounting")]
#[get("/finance/series?<months>")]
pub async fn finance_series(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    months: Option<u32>,
) -> ApiResult<Json<FinanceSeriesResp>> {
    user.require(Permission::LedgerRead)?;
    let points = crate::finance::series(&db, scope.tenant_id, months.unwrap_or(12)).await?;
    Ok(Json(FinanceSeriesResp {
        months: points.iter().map(|p| p.month.clone()).collect(),
        rent_due_cents: points.iter().map(|p| p.rent_due_cents).collect(),
        rent_collected_cents: points.iter().map(|p| p.rent_collected_cents).collect(),
        noi_cents: points.iter().map(|p| p.noi_cents).collect(),
        occupancy_bps: points.iter().map(|p| p.occupancy_bps).collect(),
        delinquency_bps: points.iter().map(|p| p.delinquency_bps).collect(),
        portfolio_value_cents: points.iter().map(|p| p.portfolio_value_cents).collect(),
        active_leases: points.iter().map(|p| p.active_leases).collect(),
    }))
}
