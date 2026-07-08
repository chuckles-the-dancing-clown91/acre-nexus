use super::{export, ReportFile, ReportTable};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, Property, Unit};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct RentRollRow {
    pub property_name: String,
    pub unit: String,
    pub tenant_name: String,
    pub rent_cents: i64,
    pub rent_label: String,
    pub term: String,
    pub status: String,
    pub payment_status: String,
    pub balance_cents: i64,
    pub balance_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct RentRollResp {
    pub generated_at: String,
    pub rows: Vec<RentRollRow>,
    pub lease_count: i32,
    pub total_rent_cents: i64,
    pub total_rent_label: String,
    pub total_balance_cents: i64,
    pub total_balance_label: String,
}

/// Build the rent roll for the tenant, optionally scoped to a property or
/// portfolio. Includes current tenancies (not ended / expired).
async fn build(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    property_id: Option<Uuid>,
    portfolio_id: Option<Uuid>,
) -> ApiResult<RentRollResp> {
    let properties = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let prop_name: HashMap<Uuid, String> =
        properties.iter().map(|p| (p.id, p.name.clone())).collect();
    // Properties in scope (portfolio filter).
    let in_scope: Option<std::collections::HashSet<Uuid>> = portfolio_id.map(|pf| {
        properties
            .iter()
            .filter(|p| p.portfolio_id == Some(pf))
            .map(|p| p.id)
            .collect()
    });

    let units = Unit::find()
        .filter(entity::unit::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let unit_name: HashMap<Uuid, String> =
        units.into_iter().map(|u| (u.id, u.unit_number)).collect();

    let mut q = Lease::find().filter(entity::lease::Column::TenantId.eq(tenant_id));
    if let Some(pid) = property_id {
        q = q.filter(entity::lease::Column::PropertyId.eq(pid));
    }
    let leases = q
        .order_by_asc(entity::lease::Column::PropertyId)
        .all(db)
        .await?;

    let mut rows = Vec::new();
    let mut total_rent = 0i64;
    let mut total_balance = 0i64;
    for l in leases {
        if matches!(l.status.as_str(), "ended" | "expired") {
            continue;
        }
        if let Some(scope) = &in_scope {
            if !scope.contains(&l.property_id) {
                continue;
            }
        }
        total_rent += l.rent_cents;
        total_balance += l.balance_cents;
        let term = match &l.end_date {
            Some(end) => format!("{} – {}", l.start_date, end),
            None => format!("{} – month-to-month", l.start_date),
        };
        rows.push(RentRollRow {
            property_name: prop_name
                .get(&l.property_id)
                .cloned()
                .unwrap_or_else(|| "—".into()),
            unit: l
                .unit_id
                .and_then(|u| unit_name.get(&u).cloned())
                .unwrap_or_else(|| "—".into()),
            tenant_name: l.tenant_name,
            rent_cents: l.rent_cents,
            rent_label: usd(l.rent_cents),
            term,
            status: l.status,
            payment_status: l.payment_status,
            balance_cents: l.balance_cents,
            balance_label: usd(l.balance_cents),
        });
    }

    Ok(RentRollResp {
        generated_at: super::today().to_string(),
        lease_count: rows.len() as i32,
        total_rent_cents: total_rent,
        total_rent_label: usd(total_rent),
        total_balance_cents: total_balance,
        total_balance_label: usd(total_balance),
        rows,
    })
}

fn to_table(r: &RentRollResp) -> ReportTable {
    ReportTable {
        title: "Rent roll".into(),
        subtitle: Some(format!(
            "As of {} · {} leases",
            r.generated_at, r.lease_count
        )),
        headers: vec![
            "Property".into(),
            "Unit".into(),
            "Tenant".into(),
            "Rent".into(),
            "Term".into(),
            "Status".into(),
            "Payment".into(),
            "Balance".into(),
        ],
        rows: r
            .rows
            .iter()
            .map(|row| {
                vec![
                    row.property_name.clone(),
                    row.unit.clone(),
                    row.tenant_name.clone(),
                    row.rent_label.clone(),
                    row.term.clone(),
                    row.status.clone(),
                    row.payment_status.clone(),
                    row.balance_label.clone(),
                ]
            })
            .collect(),
        totals: Some(vec![
            "TOTAL".into(),
            String::new(),
            String::new(),
            r.total_rent_label.clone(),
            String::new(),
            String::new(),
            String::new(),
            r.total_balance_label.clone(),
        ]),
    }
}

/// `GET /reports/rent-roll?<property_id>&<portfolio_id>` — the rent roll.
#[rocket_okapi::openapi(tag = "Reports")]
#[get("/reports/rent-roll?<property_id>&<portfolio_id>")]
pub async fn rent_roll(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    property_id: Option<String>,
    portfolio_id: Option<String>,
) -> ApiResult<Json<RentRollResp>> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    let (pid, pf) = super::parse_scope(property_id, portfolio_id)?;
    Ok(Json(build(&db, scope.tenant_id, pid, pf).await?))
}

/// `GET /reports/rent-roll/export?<format>&<property_id>&<portfolio_id>`.
#[rocket_okapi::openapi(skip)]
#[get("/reports/rent-roll/export?<format>&<property_id>&<portfolio_id>")]
pub async fn rent_roll_export(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    format: Option<String>,
    property_id: Option<String>,
    portfolio_id: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::ReportRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "reports").await?;
    let (pid, pf) = super::parse_scope(property_id, portfolio_id)?;
    let report = build(&db, scope.tenant_id, pid, pf).await?;
    export(
        &to_table(&report),
        "rent-roll",
        format.as_deref().unwrap_or("csv"),
    )
}
