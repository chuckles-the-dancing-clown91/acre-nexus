use super::{days_past_due, export, ReportFile, ReportTable};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, LeasePayment, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

/// Outstanding receivable statuses (everything not settled).
const OUTSTANDING: &[&str] = &["due", "late", "partial", "failed"];

#[derive(Serialize, Default, schemars::JsonSchema)]
pub struct AgingBuckets {
    pub current_cents: i64,
    pub d1_30_cents: i64,
    pub d31_60_cents: i64,
    pub d61_90_cents: i64,
    pub over90_cents: i64,
    pub total_cents: i64,
}

impl AgingBuckets {
    fn add(&mut self, days: i64, amount: i64) {
        match days {
            d if d <= 0 => self.current_cents += amount,
            1..=30 => self.d1_30_cents += amount,
            31..=60 => self.d31_60_cents += amount,
            61..=90 => self.d61_90_cents += amount,
            _ => self.over90_cents += amount,
        }
        self.total_cents += amount;
    }
    fn merge(&mut self, o: &AgingBuckets) {
        self.current_cents += o.current_cents;
        self.d1_30_cents += o.d1_30_cents;
        self.d31_60_cents += o.d31_60_cents;
        self.d61_90_cents += o.d61_90_cents;
        self.over90_cents += o.over90_cents;
        self.total_cents += o.total_cents;
    }
    fn cells(&self) -> Vec<String> {
        vec![
            usd(self.current_cents),
            usd(self.d1_30_cents),
            usd(self.d31_60_cents),
            usd(self.d61_90_cents),
            usd(self.over90_cents),
            usd(self.total_cents),
        ]
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct AgingRow {
    pub tenant_name: String,
    pub property_name: String,
    #[serde(flatten)]
    pub buckets: AgingBuckets,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct AgingResp {
    pub generated_at: String,
    pub rows: Vec<AgingRow>,
    #[serde(flatten)]
    pub totals: AgingBuckets,
}

async fn build(db: &crate::db::RequestDb, tenant_id: Uuid) -> ApiResult<AgingResp> {
    let today = super::today();
    let prop_name: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let leases: HashMap<Uuid, entity::lease::Model> = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|l| (l.id, l))
        .collect();

    let payments = LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(tenant_id))
        .filter(entity::lease_payment::Column::Status.is_in(OUTSTANDING.to_vec()))
        .all(db)
        .await?;

    let mut per_lease: HashMap<Uuid, AgingBuckets> = HashMap::new();
    for p in payments {
        let days = days_past_due(&p.due_date, today);
        per_lease
            .entry(p.lease_id)
            .or_default()
            .add(days, p.amount_cents);
    }

    let mut rows: Vec<AgingRow> = Vec::new();
    let mut totals = AgingBuckets::default();
    for (lease_id, buckets) in per_lease {
        if buckets.total_cents == 0 {
            continue;
        }
        totals.merge(&buckets);
        let lease = leases.get(&lease_id);
        rows.push(AgingRow {
            tenant_name: lease
                .map(|l| l.tenant_name.clone())
                .unwrap_or_else(|| "—".into()),
            property_name: lease
                .and_then(|l| prop_name.get(&l.property_id).cloned())
                .unwrap_or_else(|| "—".into()),
            buckets,
        });
    }
    rows.sort_by(|a, b| b.buckets.total_cents.cmp(&a.buckets.total_cents));

    Ok(AgingResp {
        generated_at: today.to_string(),
        rows,
        totals,
    })
}

fn to_table(r: &AgingResp) -> ReportTable {
    let headers = vec![
        "Tenant".into(),
        "Property".into(),
        "Current".into(),
        "1–30".into(),
        "31–60".into(),
        "61–90".into(),
        "90+".into(),
        "Total".into(),
    ];
    let rows = r
        .rows
        .iter()
        .map(|row| {
            let mut cells = vec![row.tenant_name.clone(), row.property_name.clone()];
            cells.extend(row.buckets.cells());
            cells
        })
        .collect();
    let mut totals = vec!["TOTAL".to_string(), String::new()];
    totals.extend(r.totals.cells());
    ReportTable {
        title: "Accounts-receivable aging".into(),
        subtitle: Some(format!("As of {}", r.generated_at)),
        headers,
        rows,
        totals: Some(totals),
    }
}

/// `GET /reports/aging` — outstanding balances by age bucket.
#[rocket_okapi::openapi(tag = "Reports")]
#[get("/reports/aging")]
pub async fn aging(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<AgingResp>> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    Ok(Json(build(&db, scope.tenant_id).await?))
}

/// `GET /reports/aging/export?<format>`.
#[rocket_okapi::openapi(skip)]
#[get("/reports/aging/export?<format>")]
pub async fn aging_export(
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
        "aging",
        format.as_deref().unwrap_or("csv"),
    )
}
