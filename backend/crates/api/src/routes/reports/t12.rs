use super::{export, ReportFile, ReportTable};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{get, State};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct T12Row {
    pub account_name: String,
    pub kind: String,
    pub monthly_cents: Vec<i64>,
    pub total_cents: i64,
    pub total_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct T12Resp {
    pub generated_at: String,
    pub entity_id: Uuid,
    /// Trailing 12 months, oldest first (`YYYY-MM`).
    pub months: Vec<String>,
    pub income: Vec<T12Row>,
    pub expenses: Vec<T12Row>,
    pub income_totals_cents: Vec<i64>,
    pub expense_totals_cents: Vec<i64>,
    pub noi_totals_cents: Vec<i64>,
    pub total_income_cents: i64,
    pub total_income_label: String,
    pub total_expense_cents: i64,
    pub total_expense_label: String,
    pub net_cents: i64,
    pub net_label: String,
}

/// Build the trailing-12-month income/expense statement for an LLC entity from
/// the general ledger.
async fn build(db: &crate::db::RequestDb, tenant_id: Uuid, entity_id: Uuid) -> ApiResult<T12Resp> {
    let months = crate::finance::month_keys(super::today(), 12);
    let n = months.len();

    // account id -> (account, per-month balances)
    let mut acc: HashMap<Uuid, (entity::ledger_account::Model, Vec<i64>)> = HashMap::new();
    for (i, m) in months.iter().enumerate() {
        let from = format!("{m}-01");
        let to = format!("{m}-31");
        let activity =
            crate::accounting::account_activity(db, tenant_id, entity_id, Some(&from), Some(&to))
                .await?;
        for a in activity {
            if matches!(a.account.kind.as_str(), "income" | "expense") {
                let bal = a.balance_cents();
                let entry = acc
                    .entry(a.account.id)
                    .or_insert_with(|| (a.account.clone(), vec![0i64; n]));
                entry.1[i] = bal;
            }
        }
    }

    let mut income: Vec<T12Row> = Vec::new();
    let mut expenses: Vec<T12Row> = Vec::new();
    for (_, (account, monthly)) in acc {
        if monthly.iter().all(|&c| c == 0) {
            continue;
        }
        let total: i64 = monthly.iter().sum();
        let row = T12Row {
            account_name: account.name.clone(),
            kind: account.kind.clone(),
            monthly_cents: monthly,
            total_cents: total,
            total_label: usd(total),
        };
        if account.kind == "income" {
            income.push(row);
        } else {
            expenses.push(row);
        }
    }
    income.sort_by(|a, b| a.account_name.cmp(&b.account_name));
    expenses.sort_by(|a, b| a.account_name.cmp(&b.account_name));

    let sum_cols = |rows: &[T12Row]| -> Vec<i64> {
        let mut totals = vec![0i64; n];
        for r in rows {
            for (i, c) in r.monthly_cents.iter().enumerate() {
                totals[i] += c;
            }
        }
        totals
    };
    let income_totals = sum_cols(&income);
    let expense_totals = sum_cols(&expenses);
    let noi_totals: Vec<i64> = income_totals
        .iter()
        .zip(&expense_totals)
        .map(|(i, e)| i - e)
        .collect();
    let total_income: i64 = income_totals.iter().sum();
    let total_expense: i64 = expense_totals.iter().sum();
    let net = total_income - total_expense;

    Ok(T12Resp {
        generated_at: super::today().to_string(),
        entity_id,
        months,
        income,
        expenses,
        income_totals_cents: income_totals,
        expense_totals_cents: expense_totals,
        noi_totals_cents: noi_totals,
        total_income_cents: total_income,
        total_income_label: usd(total_income),
        total_expense_cents: total_expense,
        total_expense_label: usd(total_expense),
        net_cents: net,
        net_label: usd(net),
    })
}

fn to_table(r: &T12Resp) -> ReportTable {
    let mut headers = vec!["Account".to_string()];
    headers.extend(r.months.iter().cloned());
    headers.push("Total".into());

    let row_cells = |name: &str, monthly: &[i64], total: i64| -> Vec<String> {
        let mut cells = vec![name.to_string()];
        cells.extend(monthly.iter().map(|c| usd(*c)));
        cells.push(usd(total));
        cells
    };

    let mut rows = Vec::new();
    rows.push(row_cells("— INCOME —", &vec![0; r.months.len()], 0));
    for row in &r.income {
        rows.push(row_cells(
            &row.account_name,
            &row.monthly_cents,
            row.total_cents,
        ));
    }
    rows.push(row_cells(
        "Total income",
        &r.income_totals_cents,
        r.total_income_cents,
    ));
    rows.push(row_cells("— EXPENSES —", &vec![0; r.months.len()], 0));
    for row in &r.expenses {
        rows.push(row_cells(
            &row.account_name,
            &row.monthly_cents,
            row.total_cents,
        ));
    }
    rows.push(row_cells(
        "Total expense",
        &r.expense_totals_cents,
        r.total_expense_cents,
    ));

    ReportTable {
        title: "T-12 income statement".into(),
        subtitle: Some(format!("Trailing 12 months · as of {}", r.generated_at)),
        headers,
        rows,
        totals: Some(row_cells(
            "NET OPERATING INCOME",
            &r.noi_totals_cents,
            r.net_cents,
        )),
    }
}

/// `GET /reports/t12?<entity>` — trailing-12-month income statement for an LLC.
#[rocket_okapi::openapi(tag = "Reports")]
#[get("/reports/t12?<entity>")]
pub async fn t12(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: &str,
) -> ApiResult<Json<T12Resp>> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    let entity_id =
        crate::routes::accounting::accounts::parse_entity(&db, scope.tenant_id, entity).await?;
    Ok(Json(build(&db, scope.tenant_id, entity_id).await?))
}

/// `GET /reports/t12/export?<entity>&<format>`.
#[rocket_okapi::openapi(skip)]
#[get("/reports/t12/export?<entity>&<format>")]
pub async fn t12_export(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: &str,
    format: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    let entity_id =
        crate::routes::accounting::accounts::parse_entity(&db, scope.tenant_id, entity).await?;
    let report = build(&db, scope.tenant_id, entity_id).await?;
    export(
        &to_table(&report),
        "t12",
        format.as_deref().unwrap_or("csv"),
    )
}
