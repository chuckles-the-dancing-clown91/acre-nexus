use super::dto::{CostLine, PropertyProfileResp, PropertyResp};
use super::helpers;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Property, PropertyDetail};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `GET /properties/<id>` — full property profile with the header dossier (hero
/// image, home breakdown, address status, rental status) and computed economics.
///
/// Economics mirror the design prototype: maintenance ≈ 9% of rent, taxes &
/// insurance ≈ 12%, management fee 8%; net = rent − those; levered figures fold
/// in mortgage debt service and best-known value.
#[rocket_okapi::openapi(tag = "Properties")]
#[get("/properties/<id>")]
pub async fn profile(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PropertyProfileResp>> {
    user.require(Permission::PropertyRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let p = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    // ---- Header dossier: home breakdown, address status, rental status ----
    let detail = PropertyDetail::find_by_id(pid).one(&db).await?;
    let home = helpers::home_breakdown(&p, detail.as_ref());
    let address_status = helpers::address_status(&p, detail.as_ref());
    let rental_status = helpers::rental_status(&db, &p).await?;

    // ---- Economics: operating + levered ----
    let rent = p.monthly_rent_cents;
    let mortgages = helpers::mortgages_for(&db, pid).await?;
    let value = helpers::best_known_value(&db, pid, p.purchase_price_cents).await?;
    let econ = helpers::economics(rent, &mortgages, value);

    let line = |label: &str, cents: i64| CostLine {
        label: label.into(),
        amount_cents: cents,
        amount_label: usd(cents),
    };

    let occupancy = format!("{}/{}", p.occupied_units, p.units);
    let mut kpis = vec![
        line("Monthly rent", rent),
        CostLine {
            label: "Occupancy".into(),
            amount_cents: p.occupied_units as i64,
            amount_label: occupancy.clone(),
        },
        line("Net revenue", econ.net_revenue_cents),
    ];
    if econ.financed {
        kpis.push(line("Cash flow after debt", econ.cash_flow_cents));
    } else {
        kpis.push(line("Maintenance MTD", econ.maintenance_cents));
    }

    let mut cost_breakdown = vec![
        line("Rent income", rent),
        line("Maintenance & repairs", -econ.maintenance_cents),
        line("Taxes & insurance", -econ.tax_cents),
        line("Management fee (8%)", -econ.mgmt_cents),
    ];
    if econ.financed {
        cost_breakdown.push(line("Debt service", -econ.debt_service_cents));
    }

    let image_url = p.image_url.clone();
    Ok(Json(PropertyProfileResp {
        property: PropertyResp::from(p),
        image_url,
        home,
        address_status,
        rental_status,
        kpis,
        cost_breakdown,
        net_revenue_cents: econ.net_revenue_cents,
        net_revenue_label: usd(econ.net_revenue_cents),
        financed: econ.financed,
        debt_service_cents: econ.debt_service_cents,
        debt_service_label: usd(econ.debt_service_cents),
        cash_flow_cents: econ.cash_flow_cents,
        cash_flow_label: usd(econ.cash_flow_cents),
        total_loan_balance_cents: econ.total_loan_balance_cents,
        total_loan_balance_label: usd(econ.total_loan_balance_cents),
        equity_cents: econ.equity_cents,
        equity_label: usd(econ.equity_cents),
    }))
}
