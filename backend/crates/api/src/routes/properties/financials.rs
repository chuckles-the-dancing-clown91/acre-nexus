use super::dto::{CostLine, LenderContact, LoanDto, PropertyFinancialsResp};
use super::helpers;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::routes::banking::dto::BankAccountResp;
use crate::routes::mortgages::dto::MortgageDto;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{BankAccount, Counterparty, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;
use uuid::Uuid;

/// `GET /properties/<id>/financials` — the Financials tab: levered economics, the
/// property's loans (each resolved to the bank that owns it and the contact
/// there), and the owning legal entity's bank accounts.
#[rocket_okapi::openapi(tag = "Financing")]
#[get("/properties/<id>/financials")]
pub async fn financials(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PropertyFinancialsResp>> {
    user.require(Permission::FinanceRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let p = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let mortgages = helpers::mortgages_for(&db, pid).await?;
    let value = helpers::best_known_value(&db, pid, p.purchase_price_cents).await?;
    let econ = helpers::economics(p.monthly_rent_cents, &mortgages, value);

    // Resolve each loan's lender (the bank that owns it) from the registry, in
    // one query keyed by id.
    let lender_ids: Vec<Uuid> = mortgages.iter().filter_map(|m| m.lender_id).collect();
    let lenders: HashMap<Uuid, LenderContact> = if lender_ids.is_empty() {
        HashMap::new()
    } else {
        Counterparty::find()
            .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::counterparty::Column::Id.is_in(lender_ids))
            .all(&db)
            .await?
            .into_iter()
            .map(|c| (c.id, LenderContact::from(c)))
            .collect()
    };

    let loans: Vec<LoanDto> = mortgages
        .into_iter()
        .map(|m| {
            let lender = m.lender_id.and_then(|lid| lenders.get(&lid).cloned());
            LoanDto {
                loan: MortgageDto::from(m),
                lender,
            }
        })
        .collect();

    // Banking information: the accounts of the LLC that holds the property.
    let bank_accounts: Vec<BankAccountResp> = match p.llc_id {
        Some(llc_id) => BankAccount::find()
            .filter(entity::bank_account::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::bank_account::Column::EntityId.eq(llc_id))
            .all(&db)
            .await?
            .into_iter()
            .map(BankAccountResp::from)
            .collect(),
        None => Vec::new(),
    };

    let line = |label: &str, cents: i64| CostLine {
        label: label.into(),
        amount_cents: cents,
        amount_label: usd(cents),
    };
    let mut cost_breakdown = vec![
        line("Rent income", p.monthly_rent_cents),
        line("Maintenance & repairs", -econ.maintenance_cents),
        line("Taxes & insurance", -econ.tax_cents),
        line("Management fee (8%)", -econ.mgmt_cents),
    ];
    if econ.financed {
        cost_breakdown.push(line("Debt service", -econ.debt_service_cents));
    }

    Ok(Json(PropertyFinancialsResp {
        property_id: pid,
        financed: econ.financed,
        net_revenue_cents: econ.net_revenue_cents,
        net_revenue_label: usd(econ.net_revenue_cents),
        debt_service_cents: econ.debt_service_cents,
        debt_service_label: usd(econ.debt_service_cents),
        cash_flow_cents: econ.cash_flow_cents,
        cash_flow_label: usd(econ.cash_flow_cents),
        total_loan_balance_cents: econ.total_loan_balance_cents,
        total_loan_balance_label: usd(econ.total_loan_balance_cents),
        equity_cents: econ.equity_cents,
        equity_label: usd(econ.equity_cents),
        cost_breakdown,
        loans,
        bank_accounts,
    }))
}
