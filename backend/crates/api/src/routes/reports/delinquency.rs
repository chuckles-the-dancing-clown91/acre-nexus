use super::{days_past_due, export, ReportFile, ReportTable};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, LeasePayment, Property, Unit};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

const OUTSTANDING: &[&str] = &["due", "late", "partial", "failed"];

#[derive(Serialize, schemars::JsonSchema)]
pub struct DelinquencyRow {
    pub tenant_name: String,
    pub property_name: String,
    pub unit: String,
    pub payment_status: String,
    pub balance_cents: i64,
    pub balance_label: String,
    pub days_late: i64,
    pub oldest_due_date: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct DelinquencyResp {
    pub generated_at: String,
    pub rows: Vec<DelinquencyRow>,
    pub tenant_count: i32,
    pub total_balance_cents: i64,
    pub total_balance_label: String,
}

async fn build(db: &crate::db::RequestDb, tenant_id: Uuid) -> ApiResult<DelinquencyResp> {
    let today = super::today();
    let prop_name: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let unit_name: HashMap<Uuid, String> = Unit::find()
        .filter(entity::unit::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.unit_number))
        .collect();

    // Outstanding payments → per-lease max days late + oldest due date.
    let mut late: HashMap<Uuid, (i64, String)> = HashMap::new();
    for p in LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::Status.is_in(OUTSTANDING.to_vec()))
        .all(db)
        .await?
    {
        let days = days_past_due(&p.due_date, today);
        let e = late.entry(p.lease_id).or_insert((days, p.due_date.clone()));
        if days > e.0 {
            e.0 = days;
        }
        if p.due_date < e.1 {
            e.1 = p.due_date.clone();
        }
    }

    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;

    let mut rows: Vec<DelinquencyRow> = Vec::new();
    let mut total = 0i64;
    for l in leases {
        if l.balance_cents <= 0 {
            continue;
        }
        total += l.balance_cents;
        let (days_late, oldest) = match late.get(&l.id) {
            Some((d, due)) => (*d, Some(due.clone())),
            None => (0, None),
        };
        rows.push(DelinquencyRow {
            tenant_name: l.tenant_name,
            property_name: prop_name
                .get(&l.property_id)
                .cloned()
                .unwrap_or_else(|| "—".into()),
            unit: l
                .unit_id
                .and_then(|u| unit_name.get(&u).cloned())
                .unwrap_or_else(|| "—".into()),
            payment_status: l.payment_status,
            balance_cents: l.balance_cents,
            balance_label: usd(l.balance_cents),
            days_late,
            oldest_due_date: oldest,
        });
    }
    rows.sort_by(|a, b| {
        b.days_late
            .cmp(&a.days_late)
            .then(b.balance_cents.cmp(&a.balance_cents))
    });

    Ok(DelinquencyResp {
        generated_at: today.to_string(),
        tenant_count: rows.len() as i32,
        total_balance_cents: total,
        total_balance_label: usd(total),
        rows,
    })
}

fn to_table(r: &DelinquencyResp) -> ReportTable {
    ReportTable {
        title: "Delinquency report".into(),
        subtitle: Some(format!(
            "As of {} · {} tenants behind",
            r.generated_at, r.tenant_count
        )),
        headers: vec![
            "Tenant".into(),
            "Property".into(),
            "Unit".into(),
            "Status".into(),
            "Balance".into(),
            "Days late".into(),
            "Oldest due".into(),
        ],
        rows: r
            .rows
            .iter()
            .map(|row| {
                vec![
                    row.tenant_name.clone(),
                    row.property_name.clone(),
                    row.unit.clone(),
                    row.payment_status.clone(),
                    row.balance_label.clone(),
                    row.days_late.to_string(),
                    row.oldest_due_date.clone().unwrap_or_else(|| "—".into()),
                ]
            })
            .collect(),
        totals: Some(vec![
            "TOTAL".into(),
            String::new(),
            String::new(),
            String::new(),
            r.total_balance_label.clone(),
            String::new(),
            String::new(),
        ]),
    }
}

/// `GET /reports/delinquency` — tenants currently behind.
#[rocket_okapi::openapi(tag = "Reports")]
#[get("/reports/delinquency")]
pub async fn delinquency(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<DelinquencyResp>> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    Ok(Json(build(&db, scope.tenant_id).await?))
}

/// `GET /reports/delinquency/export?<format>`.
#[rocket_okapi::openapi(skip)]
#[get("/reports/delinquency/export?<format>")]
pub async fn delinquency_export(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    format: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    let report = build(&db, scope.tenant_id).await?;
    export(
        &to_table(&report),
        "delinquency",
        format.as_deref().unwrap_or("csv"),
    )
}
