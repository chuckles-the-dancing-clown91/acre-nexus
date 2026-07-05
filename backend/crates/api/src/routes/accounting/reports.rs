use super::dto::{
    IncomeStatementResp, StatementLine, TrialBalanceResp, TrialBalanceRow, TrustReconciliationResp,
};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{get, State};

/// `GET /accounting/trial-balance?entity=<llc>` — every account's lifetime
/// debit/credit totals. A healthy ledger's totals are equal.
#[rocket_okapi::openapi(tag = "Accounting")]
#[get("/accounting/trial-balance?<entity>")]
pub async fn trial_balance(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: &str,
) -> ApiResult<Json<TrialBalanceResp>> {
    user.require(Permission::LedgerRead)?;
    let entity_id = super::accounts::parse_entity(&db, scope.tenant_id, entity).await?;
    let mut activity =
        crate::accounting::account_activity(&db, scope.tenant_id, entity_id, None, None).await?;
    activity.sort_by(|a, b| a.account.code.cmp(&b.account.code));

    let total_debits: i64 = activity.iter().map(|a| a.debit_cents).sum();
    let total_credits: i64 = activity.iter().map(|a| a.credit_cents).sum();
    let rows = activity
        .into_iter()
        .filter(|a| a.debit_cents != 0 || a.credit_cents != 0)
        .map(|a| TrialBalanceRow {
            code: a.account.code.clone(),
            name: a.account.name.clone(),
            kind: a.account.kind.clone(),
            debit_cents: a.debit_cents,
            credit_cents: a.credit_cents,
            debit_label: usd(a.debit_cents),
            credit_label: usd(a.credit_cents),
        })
        .collect();

    Ok(Json(TrialBalanceResp {
        entity_id,
        rows,
        total_debits_cents: total_debits,
        total_credits_cents: total_credits,
        balanced: total_debits == total_credits,
    }))
}

/// `GET /accounting/income-statement?entity=<llc>&from=&to=` — income vs
/// expenses for a period (dates inclusive, `YYYY-MM-DD`; open-ended when
/// omitted).
#[rocket_okapi::openapi(tag = "Accounting")]
#[get("/accounting/income-statement?<entity>&<from>&<to>")]
pub async fn income_statement(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: &str,
    from: Option<String>,
    to: Option<String>,
) -> ApiResult<Json<IncomeStatementResp>> {
    user.require(Permission::LedgerRead)?;
    let entity_id = super::accounts::parse_entity(&db, scope.tenant_id, entity).await?;
    let activity = crate::accounting::account_activity(
        &db,
        scope.tenant_id,
        entity_id,
        from.as_deref(),
        to.as_deref(),
    )
    .await?;

    let mut income = vec![];
    let mut expenses = vec![];
    let mut total_income = 0i64;
    let mut total_expenses = 0i64;
    for a in activity {
        let balance = a.balance_cents();
        if balance == 0 {
            continue;
        }
        match a.account.kind.as_str() {
            "income" => {
                total_income += balance;
                income.push(StatementLine {
                    name: a.account.name,
                    amount_cents: balance,
                    amount_label: usd(balance),
                });
            }
            "expense" => {
                total_expenses += balance;
                expenses.push(StatementLine {
                    name: a.account.name,
                    amount_cents: balance,
                    amount_label: usd(balance),
                });
            }
            _ => {}
        }
    }
    let net = total_income - total_expenses;

    Ok(Json(IncomeStatementResp {
        entity_id,
        from,
        to,
        income,
        expenses,
        total_income_cents: total_income,
        total_expenses_cents: total_expenses,
        net_cents: net,
        net_label: usd(net),
    }))
}

/// `GET /accounting/trust-reconciliation?entity=<llc>` — escrow cash on hand
/// vs what is owed back to residents/owners. Reconciled means equal.
#[rocket_okapi::openapi(tag = "Accounting")]
#[get("/accounting/trust-reconciliation?<entity>")]
pub async fn trust_reconciliation(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: &str,
) -> ApiResult<Json<TrustReconciliationResp>> {
    user.require(Permission::LedgerRead)?;
    let entity_id = super::accounts::parse_entity(&db, scope.tenant_id, entity).await?;
    let recon = crate::accounting::trust_reconciliation(&db, scope.tenant_id, entity_id).await?;
    Ok(Json(TrustReconciliationResp {
        entity_id,
        trust_bank_cents: recon.trust_bank_cents,
        trust_liability_cents: recon.trust_liability_cents,
        difference_cents: recon.difference_cents(),
        trust_bank_label: usd(recon.trust_bank_cents),
        trust_liability_label: usd(recon.trust_liability_cents),
        reconciled: recon.difference_cents() == 0,
    }))
}
