use super::dto::{CostLine, PropertyProfileResp, PropertyResp};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Mortgage, Property, PropertyValuation};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>` — full property profile with computed economics.
///
/// Economics mirror the design prototype: maintenance ≈ 9% of rent, taxes &
/// insurance ≈ 12%, management fee 8%; net = rent − those.
#[rocket_okapi::openapi(tag = "Properties")]
#[get("/properties/<id>")]
pub async fn profile(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PropertyProfileResp>> {
    user.require(Permission::PropertyRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let p = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let rent = p.monthly_rent_cents;
    let maint = (rent as f64 * 0.09).round() as i64;
    let tax = (rent as f64 * 0.12).round() as i64;
    let mgmt = (rent as f64 * 0.08).round() as i64;
    let net = rent - maint - tax - mgmt;

    let line = |label: &str, cents: i64| CostLine {
        label: label.into(),
        amount_cents: cents,
        amount_label: usd(cents),
    };

    // ---- Financing: debt service, levered cash flow, equity ----
    let mortgages = Mortgage::find()
        .filter(entity::mortgage::Column::PropertyId.eq(pid))
        .all(&state.db)
        .await?;
    let active: Vec<_> = mortgages
        .iter()
        .filter(|m| m.status != "paid_off")
        .collect();
    let debt_service: i64 = active
        .iter()
        .map(|m| m.monthly_payment_cents.unwrap_or(0) + m.escrow_monthly_cents.unwrap_or(0))
        .sum();
    let total_loan_balance: i64 = active
        .iter()
        .map(|m| m.current_balance_cents.unwrap_or(0))
        .sum();
    let financed = !active.is_empty();
    let cash_flow = net - debt_service;

    // Best-known value for equity: latest AVM estimate, else purchase price.
    let latest_value = PropertyValuation::find()
        .filter(entity::property_valuation::Column::PropertyId.eq(pid))
        .order_by_desc(entity::property_valuation::Column::CreatedAt)
        .one(&state.db)
        .await?
        .and_then(|v| v.estimated_value_cents)
        .or(p.purchase_price_cents)
        .unwrap_or(0);
    let equity = latest_value - total_loan_balance;

    let occupancy = format!("{}/{}", p.occupied_units, p.units);
    let mut kpis = vec![
        line("Monthly rent", rent),
        CostLine {
            label: "Occupancy".into(),
            amount_cents: p.occupied_units as i64,
            amount_label: occupancy.clone(),
        },
        line("Net revenue", net),
    ];
    if financed {
        kpis.push(line("Cash flow after debt", cash_flow));
    } else {
        kpis.push(line("Maintenance MTD", maint));
    }

    let mut cost_breakdown = vec![
        line("Rent income", rent),
        line("Maintenance & repairs", -maint),
        line("Taxes & insurance", -tax),
        line("Management fee (8%)", -mgmt),
    ];
    if financed {
        cost_breakdown.push(line("Debt service", -debt_service));
    }

    Ok(Json(PropertyProfileResp {
        property: PropertyResp::from(p),
        kpis,
        cost_breakdown,
        net_revenue_cents: net,
        net_revenue_label: usd(net),
        financed,
        debt_service_cents: debt_service,
        debt_service_label: usd(debt_service),
        cash_flow_cents: cash_flow,
        cash_flow_label: usd(cash_flow),
        total_loan_balance_cents: total_loan_balance,
        total_loan_balance_label: usd(total_loan_balance),
        equity_cents: equity,
        equity_label: usd(equity),
    }))
}
