use super::{export, ReportFile, ReportTable};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{Datelike, NaiveDate};
use entity::prelude::Llc;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct StatementLine {
    pub name: String,
    pub amount_cents: i64,
    pub amount_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct OwnerStatementResp {
    pub generated_at: String,
    pub entity_id: Uuid,
    pub entity_name: String,
    pub period_start: String,
    pub period_end: String,
    pub rent_collected_cents: i64,
    pub rent_collected_label: String,
    /// Operating expenses broken out by ledger account (management fee apart).
    pub expense_lines: Vec<StatementLine>,
    pub expenses_cents: i64,
    pub expenses_label: String,
    pub mgmt_fee_cents: i64,
    pub mgmt_fee_label: String,
    pub net_cents: i64,
    pub net_label: String,
}

/// Resolve the reporting period: explicit `from`/`to`, else the current
/// calendar month to date.
fn resolve_period(from: Option<String>, to: Option<String>) -> ApiResult<(String, String)> {
    let parse = |s: &str| -> ApiResult<NaiveDate> {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| ApiError::BadRequest("dates must be YYYY-MM-DD".into()))
    };
    let today = super::today();
    let start = match from.filter(|s| !s.is_empty()) {
        Some(s) => parse(&s)?,
        None => today.with_day(1).unwrap_or(today),
    };
    let end = match to.filter(|s| !s.is_empty()) {
        Some(s) => parse(&s)?,
        None => today,
    };
    if end < start {
        return Err(ApiError::BadRequest("to must not precede from".into()));
    }
    Ok((start.to_string(), end.to_string()))
}

async fn build(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    entity_id: Uuid,
    period_start: &str,
    period_end: &str,
) -> ApiResult<OwnerStatementResp> {
    let llc = Llc::find_by_id(entity_id)
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))?;

    let act =
        crate::payouts::gather_period(db, tenant_id, entity_id, period_start, period_end).await?;
    let mgmt_fee_bps =
        crate::settings::get_i64(db, tenant_id, crate::settings::PAYOUT_MGMT_FEE_BPS).await;
    let amounts =
        crate::payouts::compute_amounts(act.rent_collected_cents, act.expenses_cents, mgmt_fee_bps);

    Ok(OwnerStatementResp {
        generated_at: super::today().to_string(),
        entity_id,
        entity_name: llc.name,
        period_start: period_start.to_string(),
        period_end: period_end.to_string(),
        rent_collected_cents: amounts.rent_collected_cents,
        rent_collected_label: usd(amounts.rent_collected_cents),
        expense_lines: act
            .expense_lines
            .into_iter()
            .map(|(name, amount_cents)| StatementLine {
                name,
                amount_cents,
                amount_label: usd(amount_cents),
            })
            .collect(),
        expenses_cents: amounts.expenses_cents,
        expenses_label: usd(amounts.expenses_cents),
        mgmt_fee_cents: amounts.mgmt_fee_cents,
        mgmt_fee_label: usd(amounts.mgmt_fee_cents),
        net_cents: amounts.net_cents,
        net_label: usd(amounts.net_cents),
    })
}

fn to_table(r: &OwnerStatementResp) -> ReportTable {
    let mut rows: Vec<Vec<String>> = vec![
        vec!["Rent collected".into(), usd(r.rent_collected_cents)],
        vec!["— Operating expenses —".into(), String::new()],
    ];
    for line in &r.expense_lines {
        rows.push(vec![
            format!("  {}", line.name),
            format!("-{}", line.amount_label),
        ]);
    }
    rows.push(vec![
        "Total operating expenses".into(),
        format!("-{}", r.expenses_label),
    ]);
    rows.push(vec![
        "Management fee".into(),
        format!("-{}", r.mgmt_fee_label),
    ]);

    ReportTable {
        title: format!("Owner statement — {}", r.entity_name),
        subtitle: Some(format!("{} to {}", r.period_start, r.period_end)),
        headers: vec!["Item".into(), "Amount".into()],
        rows,
        totals: Some(vec!["NET OWNER DRAW".into(), r.net_label.clone()]),
    }
}

/// `GET /reports/owner-statement?<entity>&<from>&<to>` — a cash-basis owner
/// statement (rent collected − operating expenses − management fee = net draw)
/// for one legal entity over a period. Reconciles with owner payouts.
#[rocket_okapi::openapi(tag = "Reports")]
#[get("/reports/owner-statement?<entity>&<from>&<to>")]
pub async fn owner_statement(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: &str,
    from: Option<String>,
    to: Option<String>,
) -> ApiResult<Json<OwnerStatementResp>> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    let entity_id =
        crate::routes::accounting::accounts::parse_entity(&db, scope.tenant_id, entity).await?;
    let (start, end) = resolve_period(from, to)?;
    Ok(Json(
        build(&db, scope.tenant_id, entity_id, &start, &end).await?,
    ))
}

/// `GET /reports/owner-statement/export?<entity>&<from>&<to>&<format>`.
#[rocket_okapi::openapi(skip)]
#[get("/reports/owner-statement/export?<entity>&<from>&<to>&<format>")]
#[allow(clippy::too_many_arguments)]
pub async fn owner_statement_export(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: &str,
    from: Option<String>,
    to: Option<String>,
    format: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    let entity_id =
        crate::routes::accounting::accounts::parse_entity(&db, scope.tenant_id, entity).await?;
    let (start, end) = resolve_period(from, to)?;
    let report = build(&db, scope.tenant_id, entity_id, &start, &end).await?;
    export(
        &to_table(&report),
        "owner-statement",
        format.as_deref().unwrap_or("pdf"),
    )
}
