//! Back-office reports — every one reads the same hours, receipts and bills:
//!
//! * **Payroll** (`payroll:read`): whole Monday–Sunday weeks per person, split
//!   by the overtime rule, gross at the frozen rates, mileage paid back.
//! * **Timesheets** (`team:read`): every entry in the period.
//! * **Profit** (`payroll:read`): each work order / rehab project with activity
//!   in the period, costed like its profit panel, with rollups by property,
//!   technician, category and month, and the owner's-eye KPIs.
//! * **Tax package** (`payroll:read`): mileage log, expense ledger, missing
//!   receipts, pay by person (W-2 vs 1099-NEC) and key dates.
//! * **Dashboard** (`team:read`, money with `payroll:read`).
//! * **Cost sheet** — a printable PDF for one work order / project.
//!
//! Each report is JSON at `/reports/<name>` and a file at
//! `/reports/<name>/export?format=csv|pdf`.

use super::costing::{preview, resolve_work};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::pdfdoc::{hm, money, pct, Block, Column, Document, Table};
use crate::rbac::Permission;
use crate::routes::reports::ReportFile;
use crate::routes::team::{parse_id, sees_pay};
use crate::tenancy::TenantScope;
use crate::workforce::costing::{self, Cost, Work};
use crate::workforce::{
    self, entry_minutes, minutes_cost, needs_review, overtime, parse_date, Rules,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use entity::prelude::{
    Document as Doc, EmployeeProfile, Expense, MaintenanceTicket, Property, RehabProject, Theme,
    TimeEntry, TimeOffRequest, User,
};
use rocket::get;
use rocket::serde::json::Json;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Shared plumbing
// ---------------------------------------------------------------------------

/// One report, ready to print or export.
pub struct Printable {
    pub title: String,
    pub subtitle: Option<String>,
    pub summary: Vec<(String, String)>,
    pub tables: Vec<(String, Table)>,
    pub landscape: bool,
    pub basename: String,
}

async fn org_name(db: &impl ConnectionTrait, tenant_id: Uuid) -> String {
    Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|t| t.company_name)
        .unwrap_or_default()
}

fn csv_cell(s: &str) -> String {
    if s.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Render to `csv` (the chosen `section`'s table, default the first) or `pdf`
/// (everything).
pub async fn export(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    p: Printable,
    format: &str,
    section: Option<&str>,
) -> ApiResult<ReportFile> {
    match format {
        "pdf" => {
            let mut blocks = Vec::new();
            if !p.summary.is_empty() {
                blocks.push(Block::KeyValues(p.summary.clone()));
            }
            for (heading, t) in p.tables {
                if !heading.is_empty() {
                    blocks.push(Block::Heading(heading));
                }
                blocks.push(Block::Table(t));
            }
            let doc = Document {
                title: p.title,
                subtitle: p.subtitle,
                organization: org_name(db, tenant_id).await,
                landscape: p.landscape,
                blocks,
            };
            Ok(ReportFile::new(
                crate::pdfdoc::render(&doc),
                "application/pdf",
                format!("{}.pdf", p.basename),
            ))
        }
        "csv" => {
            let idx = match section {
                Some(s) => p
                    .tables
                    .iter()
                    .position(|(h, _)| slug(h) == s)
                    .ok_or_else(|| ApiError::BadRequest(format!("no section '{s}'")))?,
                None => 0,
            };
            let (heading, t) = p
                .tables
                .into_iter()
                .nth(idx)
                .ok_or_else(|| ApiError::BadRequest("nothing to export".into()))?;
            let line = |cells: &[String]| {
                cells
                    .iter()
                    .map(|c| csv_cell(c))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let mut out = line(
                &t.columns
                    .iter()
                    .map(|c| c.label.clone())
                    .collect::<Vec<_>>(),
            );
            out.push('\n');
            for r in t.rows.iter().chain(t.totals.iter()) {
                out.push_str(&line(r));
                out.push('\n');
            }
            let suffix = if heading.is_empty() {
                String::new()
            } else {
                format!("-{}", slug(&heading))
            };
            Ok(ReportFile::new(
                out.into_bytes(),
                "text/csv",
                format!("{}{suffix}.csv", p.basename),
            ))
        }
        other => Err(ApiError::BadRequest(format!(
            "unsupported format: {other} (expected csv or pdf)"
        ))),
    }
}

/// A section heading as a URL-safe key (`Mileage log` → `mileage-log`).
pub fn slug(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn period(
    from: Option<&str>,
    to: Option<&str>,
    rules: &Rules,
) -> ApiResult<(NaiveDate, NaiveDate)> {
    let today = rules.local_date(Utc::now().into());
    let from = match from.filter(|s| !s.is_empty()) {
        Some(s) => parse_date(s, "from")?,
        None => today.with_day(1).expect("day 1"),
    };
    let to = match to.filter(|s| !s.is_empty()) {
        Some(s) => parse_date(s, "to")?,
        None => today,
    };
    if to < from {
        return Err(ApiError::BadRequest("to is before from".into()));
    }
    if (to - from).num_days() > 800 {
        return Err(ApiError::BadRequest("pick at most two years".into()));
    }
    Ok((from, to))
}

fn range_label(from: NaiveDate, to: NaiveDate) -> String {
    format!(
        "{} – {}",
        from.format("%b %-d, %Y"),
        to.format("%b %-d, %Y")
    )
}

async fn names(
    db: &impl ConnectionTrait,
    ids: impl IntoIterator<Item = Uuid>,
) -> ApiResult<HashMap<Uuid, String>> {
    let ids: Vec<Uuid> = ids
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    Ok(User::find()
        .filter(entity::user::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect())
}

fn employment_label(t: &str) -> &'static str {
    match t {
        "part_time" => "Part-time",
        "seasonal" => "Seasonal",
        "contractor" => "Contractor (1099)",
        _ => "Full-time",
    }
}

// ---------------------------------------------------------------------------
// Payroll
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct PayrollRow {
    pub user_id: Uuid,
    pub name: String,
    pub week_of: String,
    pub days_worked: usize,
    pub entries: usize,
    pub minutes: i64,
    pub regular_minutes: i64,
    pub overtime_minutes: i64,
    pub double_minutes: i64,
    pub rate_cents: i64,
    pub gross_cents: i64,
    pub premium_cents: i64,
    pub mileage_paid_back_cents: i64,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PayrollReport {
    pub from: String,
    pub to: String,
    pub overtime_rule: String,
    pub approved_only: bool,
    pub rows: Vec<PayrollRow>,
    pub total_minutes: i64,
    pub total_gross_cents: i64,
    pub total_mileage_paid_back_cents: i64,
    /// Entries left out (not approved / missed punch), when approved-only.
    pub excluded: Vec<String>,
}

pub(crate) async fn payroll_data(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    approved_only: bool,
    rules: &Rules,
) -> ApiResult<PayrollReport> {
    let mut entries = workforce::entries_for_weeks(db, tenant_id, from, to, None, rules).await?;
    let mut excluded = Vec::new();
    let who = names(db, entries.iter().map(|e| e.user_id)).await?;
    let now = Utc::now();
    entries.retain(|e| {
        if needs_review(e) {
            excluded.push(format!(
                "{} — {} missed punch on {}",
                who.get(&e.user_id).cloned().unwrap_or_default(),
                hm(entry_minutes(e, now)),
                rules.local_date(e.started_at)
            ));
            return false;
        }
        if approved_only && e.approved_at.is_none() {
            excluded.push(format!(
                "{} — {} not approved on {}",
                who.get(&e.user_id).cloned().unwrap_or_default(),
                hm(entry_minutes(e, now)),
                rules.local_date(e.started_at)
            ));
            return false;
        }
        true
    });
    let profiles = workforce::profiles_by_user(db, tenant_id).await?;
    let (weeks, _) = workforce::week_splits(&entries, &profiles, rules);
    // Own-vehicle mileage paid back, by person and week.
    let first = overtime::week_start(from);
    let last = overtime::week_start(to) + Duration::days(6);
    let mut mileage: HashMap<(Uuid, NaiveDate), i64> = HashMap::new();
    for x in Expense::find()
        .filter(entity::expense::Column::TenantId.eq(tenant_id))
        .filter(entity::expense::Column::Reimbursable.eq(true))
        .filter(entity::expense::Column::IncurredOn.gte(first.to_string()))
        .filter(entity::expense::Column::IncurredOn.lte(last.to_string()))
        .all(db)
        .await?
    {
        if let (Some(u), Ok(d)) = (x.user_id, x.incurred_on.parse::<NaiveDate>()) {
            *mileage.entry((u, overtime::week_start(d))).or_default() += x.amount_cents;
        }
    }
    let mut rows: Vec<PayrollRow> = weeks
        .into_iter()
        .map(|w| PayrollRow {
            user_id: w.user_id,
            name: who.get(&w.user_id).cloned().unwrap_or_default(),
            week_of: w.monday.to_string(),
            days_worked: w.days_worked,
            entries: w.entries,
            minutes: w.split.minutes(),
            regular_minutes: w.split.regular,
            overtime_minutes: w.split.overtime,
            double_minutes: w.split.double,
            rate_cents: w.rate_cents,
            gross_cents: w.gross_cents,
            premium_cents: w.premium_cents,
            mileage_paid_back_cents: mileage.get(&(w.user_id, w.monday)).copied().unwrap_or(0),
        })
        .collect();
    rows.sort_by(|a, b| a.week_of.cmp(&b.week_of).then(a.name.cmp(&b.name)));
    Ok(PayrollReport {
        from: first.to_string(),
        to: last.to_string(),
        overtime_rule: rules.overtime.describe().into(),
        approved_only,
        total_minutes: rows.iter().map(|r| r.minutes).sum(),
        total_gross_cents: rows.iter().map(|r| r.gross_cents).sum(),
        total_mileage_paid_back_cents: rows.iter().map(|r| r.mileage_paid_back_cents).sum(),
        rows,
        excluded,
    })
}

fn payroll_printable(r: &PayrollReport) -> Printable {
    let rows = r
        .rows
        .iter()
        .map(|x| {
            vec![
                x.name.clone(),
                x.week_of.clone(),
                x.days_worked.to_string(),
                x.entries.to_string(),
                hm(x.minutes),
                hm(x.regular_minutes),
                hm(x.overtime_minutes),
                hm(x.double_minutes),
                money(x.rate_cents),
                money(x.gross_cents),
                money(x.mileage_paid_back_cents),
            ]
        })
        .collect();
    let mut tables = vec![(
        String::new(),
        Table {
            columns: vec![
                Column::left("Employee", 18.0),
                Column::left("Week of", 10.0),
                Column::right("Days", 5.0),
                Column::right("Entries", 6.0),
                Column::right("Hours", 7.0),
                Column::right("Regular", 7.0),
                Column::right("OT 1.5×", 7.0),
                Column::right("DT 2×", 6.0),
                Column::right("Rate", 8.0),
                Column::right("Gross pay", 10.0),
                Column::right("Mileage back", 10.0),
            ],
            rows,
            totals: Some(vec![
                "Total".into(),
                String::new(),
                String::new(),
                String::new(),
                hm(r.total_minutes),
                hm(r.rows.iter().map(|x| x.regular_minutes).sum()),
                hm(r.rows.iter().map(|x| x.overtime_minutes).sum()),
                hm(r.rows.iter().map(|x| x.double_minutes).sum()),
                String::new(),
                money(r.total_gross_cents),
                money(r.total_mileage_paid_back_cents),
            ]),
        },
    )];
    if !r.excluded.is_empty() {
        tables.push((
            "Left out".into(),
            Table {
                columns: vec![Column::left("Entry and why", 1.0)],
                rows: r.excluded.iter().map(|e| vec![e.clone()]).collect(),
                totals: None,
            },
        ));
    }
    Printable {
        title: "Payroll".into(),
        subtitle: Some(format!(
            "Weeks of {} – {} · {}{}",
            r.from,
            r.to,
            r.overtime_rule,
            if r.approved_only {
                " · approved time only"
            } else {
                ""
            }
        )),
        summary: vec![
            ("Hours".into(), hm(r.total_minutes)),
            ("Gross pay".into(), money(r.total_gross_cents)),
            (
                "Mileage paid back".into(),
                money(r.total_mileage_paid_back_cents),
            ),
        ],
        tables,
        landscape: true,
        basename: format!("payroll-{}", r.from),
    }
}

/// `GET /reports/payroll?from&to&approved_only`.
#[rocket_okapi::openapi(tag = "Back office reports")]
#[get("/reports/payroll?<from>&<to>&<approved_only>")]
pub async fn payroll(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
    approved_only: Option<bool>,
) -> ApiResult<Json<PayrollReport>> {
    user.require(Permission::PayrollRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (f, t) = period(from.as_deref(), to.as_deref(), &rules)?;
    Ok(Json(
        payroll_data(
            &db,
            scope.tenant_id,
            f,
            t,
            approved_only.unwrap_or(false),
            &rules,
        )
        .await?,
    ))
}

/// `GET /reports/payroll/export?from&to&approved_only&format`.
#[rocket_okapi::openapi(skip)]
#[get("/reports/payroll/export?<from>&<to>&<approved_only>&<format>")]
pub async fn payroll_export(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
    approved_only: Option<bool>,
    format: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::PayrollRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (f, t) = period(from.as_deref(), to.as_deref(), &rules)?;
    let r = payroll_data(
        &db,
        scope.tenant_id,
        f,
        t,
        approved_only.unwrap_or(false),
        &rules,
    )
    .await?;
    export(
        &db,
        scope.tenant_id,
        payroll_printable(&r),
        format.as_deref().unwrap_or("pdf"),
        None,
    )
    .await
}

// ---------------------------------------------------------------------------
// Timesheets
// ---------------------------------------------------------------------------

async fn timesheet_printable(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    user_id: Option<Uuid>,
    rules: &Rules,
) -> ApiResult<Printable> {
    let mut q = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::StartedAt.gte(rules.day_start(from)))
        .filter(entity::time_entry::Column::StartedAt.lt(rules.day_start(to + Duration::days(1))));
    if let Some(u) = user_id {
        q = q.filter(entity::time_entry::Column::UserId.eq(u));
    }
    let entries = q
        .order_by_asc(entity::time_entry::Column::StartedAt)
        .all(db)
        .await?;
    let dtos = crate::routes::team::entry_dtos(db, tenant_id, entries, false, rules).await?;
    let fmt_t = |s: &str| {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&rules.tz).format("%-I:%M %p").to_string())
            .unwrap_or_default()
    };
    let fmt_d = |s: &str| {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|d| d.with_timezone(&rules.tz).format("%a %b %-d").to_string())
            .unwrap_or_default()
    };
    let total: i64 = dtos.iter().map(|d| d.minutes).sum();
    let rows = dtos
        .iter()
        .map(|d| {
            let work = d
                .work_order_title
                .clone()
                .or(d.project_name.clone())
                .unwrap_or_else(|| d.kind.replace('_', " "));
            vec![
                fmt_d(&d.started_at),
                d.user_name.clone(),
                work,
                d.property_name.clone().unwrap_or_default(),
                fmt_t(&d.started_at),
                d.ended_at
                    .as_deref()
                    .map(fmt_t)
                    .unwrap_or_else(|| "on the clock".into()),
                if d.break_minutes > 0 {
                    format!("{} min", d.break_minutes)
                } else {
                    String::new()
                },
                hm(d.minutes),
                if d.needs_review {
                    "missed punch".into()
                } else if d.approved {
                    "approved".into()
                } else {
                    "waiting".into()
                },
            ]
        })
        .collect();
    Ok(Printable {
        title: "Timesheets".into(),
        subtitle: Some(range_label(from, to)),
        summary: vec![("Hours".into(), hm(total))],
        tables: vec![(
            String::new(),
            Table {
                columns: vec![
                    Column::left("Date", 9.0),
                    Column::left("Person", 13.0),
                    Column::left("Work", 22.0),
                    Column::left("Property", 14.0),
                    Column::right("In", 7.0),
                    Column::right("Out", 8.0),
                    Column::right("Break", 6.0),
                    Column::right("Hours", 6.0),
                    Column::left("Status", 9.0),
                ],
                rows,
                totals: Some(vec![
                    "Total".into(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    hm(total),
                    String::new(),
                ]),
            },
        )],
        landscape: true,
        basename: format!("timesheets-{from}"),
    })
}

/// `GET /reports/timesheets/export?from&to&user_id&format`.
#[rocket_okapi::openapi(skip)]
#[get("/reports/timesheets/export?<from>&<to>&<user_id>&<format>")]
pub async fn timesheets_export(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
    user_id: Option<String>,
    format: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::TeamRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (f, t) = period(from.as_deref(), to.as_deref(), &rules)?;
    let uid = match user_id.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => Some(parse_id(s, "user")?),
        None => None,
    };
    let p = timesheet_printable(&db, scope.tenant_id, f, t, uid, &rules).await?;
    export(
        &db,
        scope.tenant_id,
        p,
        format.as_deref().unwrap_or("pdf"),
        None,
    )
    .await
}

// ---------------------------------------------------------------------------
// Profit (work-order costing across the period)
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct WorkRow {
    /// `work_order` | `project`
    pub kind: String,
    pub id: Uuid,
    pub title: String,
    pub property: String,
    pub category: String,
    pub status: String,
    pub month: String,
    #[serde(flatten)]
    pub cost: Cost,
}

#[derive(Serialize, schemars::JsonSchema, Default, Clone)]
pub struct Rollup {
    pub key: String,
    pub label: String,
    pub jobs: f64,
    pub minutes: i64,
    pub revenue_cents: i64,
    pub costs_cents: i64,
    pub gross_cents: i64,
    pub gross_bps: i64,
    pub revenue_per_hour_cents: i64,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ProfitReport {
    pub from: String,
    pub to: String,
    pub target_margin_bps: i64,
    pub work: Vec<WorkRow>,
    pub by_property: Vec<Rollup>,
    pub by_technician: Vec<Rollup>,
    pub by_category: Vec<Rollup>,
    pub by_month: Vec<Rollup>,
    pub revenue_cents: i64,
    pub costs_cents: i64,
    pub gross_cents: i64,
    pub gross_bps: i64,
    pub net_cents: i64,
    pub minutes: i64,
    pub revenue_per_hour_cents: i64,
    pub unbilled_cents: i64,
    pub vendor_bills_cents: i64,
    pub under_target: usize,
}

fn add(bucket: &mut BTreeMap<String, Rollup>, key: &str, label: &str, c: &Cost, share: f64) {
    let r = bucket.entry(key.to_string()).or_insert_with(|| Rollup {
        key: key.into(),
        label: label.into(),
        ..Default::default()
    });
    r.jobs += share;
    r.minutes += (c.minutes as f64 * share).round() as i64;
    r.revenue_cents += (c.revenue_cents as f64 * share).round() as i64;
    r.costs_cents += (c.costs_cents as f64 * share).round() as i64;
    r.gross_cents += (c.gross_cents as f64 * share).round() as i64;
}

fn finish(bucket: BTreeMap<String, Rollup>) -> Vec<Rollup> {
    let mut v: Vec<Rollup> = bucket
        .into_values()
        .map(|mut r| {
            r.gross_bps = costing::bps(r.gross_cents, r.revenue_cents);
            r.revenue_per_hour_cents = if r.minutes > 0 {
                r.revenue_cents * 60 / r.minutes
            } else {
                0
            };
            r.jobs = (r.jobs * 100.0).round() / 100.0;
            r
        })
        .collect();
    v.sort_by(|a, b| b.revenue_cents.cmp(&a.revenue_cents));
    v
}

pub(crate) async fn profit_data(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    rules: &Rules,
) -> ApiResult<ProfitReport> {
    let start = rules.day_start(from);
    let end = rules.day_start(to + Duration::days(1));
    // Work with activity in the period: time, expenses, or resolved in it.
    let mut work: HashSet<Work> = HashSet::new();
    for e in TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::StartedAt.gte(start))
        .filter(entity::time_entry::Column::StartedAt.lt(end))
        .all(db)
        .await?
    {
        if let Some(t) = e.maintenance_ticket_id {
            work.insert(Work::Ticket(t));
        } else if let Some(p) = e.rehab_project_id {
            work.insert(Work::Project(p));
        }
    }
    for x in Expense::find()
        .filter(entity::expense::Column::TenantId.eq(tenant_id))
        .filter(entity::expense::Column::IncurredOn.gte(from.to_string()))
        .filter(entity::expense::Column::IncurredOn.lte(to.to_string()))
        .all(db)
        .await?
    {
        if let Some(t) = x.maintenance_ticket_id {
            work.insert(Work::Ticket(t));
        } else if let Some(p) = x.rehab_project_id {
            work.insert(Work::Project(p));
        }
    }
    for t in MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::ResolvedAt.gte(start))
        .filter(entity::maintenance_ticket::Column::ResolvedAt.lt(end))
        .all(db)
        .await?
    {
        work.insert(Work::Ticket(t.id));
    }
    let list: Vec<Work> = work.into_iter().collect();
    let costs = costing::cost_work(db, tenant_id, &list, rules).await?;
    let ticket_ids: Vec<Uuid> = list
        .iter()
        .filter_map(|w| match w {
            Work::Ticket(t) => Some(*t),
            _ => None,
        })
        .collect();
    let project_ids: Vec<Uuid> = list
        .iter()
        .filter_map(|w| match w {
            Work::Project(p) => Some(*p),
            _ => None,
        })
        .collect();
    let tickets: HashMap<Uuid, entity::maintenance_ticket::Model> = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::Id.is_in(ticket_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let projects: HashMap<Uuid, entity::rehab_project::Model> = RehabProject::find()
        .filter(entity::rehab_project::Column::Id.is_in(project_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();
    let props: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let tech_names = names(
        db,
        costs.values().flat_map(|c| c.minutes_by.keys().copied()),
    )
    .await?;

    let mut rows = Vec::new();
    let (mut by_property, mut by_tech, mut by_cat, mut by_month) = (
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    for (w, c) in costs {
        let (kind, id, title, property_id, category, status, when) = match w {
            Work::Ticket(t) => {
                let Some(tk) = tickets.get(&t) else { continue };
                (
                    "work_order",
                    t,
                    tk.title.clone(),
                    tk.property_id,
                    tk.category.clone(),
                    tk.status.clone(),
                    tk.resolved_at.map(|d| rules.local_date(d)).unwrap_or(to),
                )
            }
            Work::Project(p) => {
                let Some(pr) = projects.get(&p) else { continue };
                (
                    "project",
                    p,
                    pr.name.clone(),
                    pr.property_id,
                    "rehab".to_string(),
                    pr.status.clone(),
                    to,
                )
            }
        };
        let property = props.get(&property_id).cloned().unwrap_or_default();
        let month = format!("{}-{:02}", when.year(), when.month());
        add(
            &mut by_property,
            &property_id.to_string(),
            &property,
            &c,
            1.0,
        );
        add(&mut by_cat, &category, &category, &c, 1.0);
        add(&mut by_month, &month, &month, &c, 1.0);
        let total_m: i64 = c.minutes_by.values().sum();
        for (u, m) in &c.minutes_by {
            if total_m > 0 {
                let name = tech_names.get(u).cloned().unwrap_or_default();
                add(
                    &mut by_tech,
                    &u.to_string(),
                    &name,
                    &c,
                    *m as f64 / total_m as f64,
                );
            }
        }
        rows.push(WorkRow {
            kind: kind.into(),
            id,
            title,
            property,
            category,
            status,
            month,
            cost: c,
        });
    }
    rows.sort_by(|a, b| b.cost.revenue_cents.cmp(&a.cost.revenue_cents));
    let sum = |f: fn(&Cost) -> i64| rows.iter().map(|r| f(&r.cost)).sum::<i64>();
    let revenue = sum(|c| c.revenue_cents);
    let gross = sum(|c| c.gross_cents);
    let minutes = sum(|c| c.minutes);
    Ok(ProfitReport {
        from: from.to_string(),
        to: to.to_string(),
        target_margin_bps: rules.target_margin_bps,
        under_target: rows
            .iter()
            .filter(|r| r.cost.revenue_cents > 0 && r.cost.gross_bps < rules.target_margin_bps)
            .count(),
        revenue_cents: revenue,
        costs_cents: sum(|c| c.costs_cents),
        gross_cents: gross,
        gross_bps: costing::bps(gross, revenue),
        net_cents: sum(|c| c.net_cents),
        minutes,
        revenue_per_hour_cents: if minutes > 0 {
            revenue * 60 / minutes
        } else {
            0
        },
        unbilled_cents: sum(|c| c.unbilled_cents),
        vendor_bills_cents: sum(|c| c.vendor_bills_cents),
        by_property: finish(by_property),
        by_technician: finish(by_tech),
        by_category: finish(by_cat),
        by_month: {
            let mut m = finish(by_month);
            m.sort_by(|a, b| a.key.cmp(&b.key));
            m
        },
        work: rows,
    })
}

fn rollup_table(rows: &[Rollup], first: &str) -> Table {
    Table {
        columns: vec![
            Column::left(first, 20.0),
            Column::right("Jobs", 6.0),
            Column::right("Hours", 7.0),
            Column::right("Revenue", 10.0),
            Column::right("Costs", 10.0),
            Column::right("Gross", 10.0),
            Column::right("Gross %", 7.0),
            Column::right("Per hour", 9.0),
        ],
        rows: rows
            .iter()
            .map(|r| {
                vec![
                    r.label.clone(),
                    format!("{}", r.jobs),
                    hm(r.minutes),
                    money(r.revenue_cents),
                    money(r.costs_cents),
                    money(r.gross_cents),
                    pct(r.gross_bps),
                    money(r.revenue_per_hour_cents),
                ]
            })
            .collect(),
        totals: None,
    }
}

fn profit_printable(r: &ProfitReport) -> Printable {
    let work = Table {
        columns: vec![
            Column::left("Work", 20.0),
            Column::left("Property", 13.0),
            Column::left("Category", 8.0),
            Column::right("Hours", 6.0),
            Column::right("Labor", 9.0),
            Column::right("Parts", 8.0),
            Column::right("Miles+exp", 8.0),
            Column::right("Costs", 9.0),
            Column::right("Billed", 9.0),
            Column::right("Unbilled", 9.0),
            Column::right("Gross", 9.0),
            Column::right("Gross %", 7.0),
        ],
        rows: r
            .work
            .iter()
            .map(|w| {
                let c = &w.cost;
                vec![
                    w.title.clone(),
                    w.property.clone(),
                    w.category.clone(),
                    hm(c.minutes),
                    money(c.labor_cents),
                    money(c.parts_cents + c.other_lines_cents),
                    money(c.mileage_cents + c.expenses_cents),
                    money(c.costs_cents),
                    money(c.billed_cents),
                    money(c.unbilled_cents),
                    money(c.gross_cents),
                    pct(c.gross_bps),
                ]
            })
            .collect(),
        totals: Some(vec![
            "Total".into(),
            String::new(),
            String::new(),
            hm(r.minutes),
            money(r.work.iter().map(|w| w.cost.labor_cents).sum()),
            money(
                r.work
                    .iter()
                    .map(|w| w.cost.parts_cents + w.cost.other_lines_cents)
                    .sum(),
            ),
            money(
                r.work
                    .iter()
                    .map(|w| w.cost.mileage_cents + w.cost.expenses_cents)
                    .sum(),
            ),
            money(r.costs_cents),
            money(r.work.iter().map(|w| w.cost.billed_cents).sum()),
            money(r.unbilled_cents),
            money(r.gross_cents),
            pct(r.gross_bps),
        ]),
    };
    Printable {
        title: "In-house maintenance profit".into(),
        subtitle: Some(format!("{} – {}", r.from, r.to)),
        summary: vec![
            ("Revenue (billed + unbilled)".into(), money(r.revenue_cents)),
            ("Costs".into(), money(r.costs_cents)),
            (
                "Gross".into(),
                format!("{} ({})", money(r.gross_cents), pct(r.gross_bps)),
            ),
            ("Net after overhead".into(), money(r.net_cents)),
            ("Labor hours".into(), hm(r.minutes)),
            (
                "Revenue per labor hour".into(),
                money(r.revenue_per_hour_cents),
            ),
            ("Not billed yet".into(), money(r.unbilled_cents)),
            (
                "Outside vendor bills (owner's cost)".into(),
                money(r.vendor_bills_cents),
            ),
            (
                "Work under the target margin".into(),
                format!("{} (target {})", r.under_target, pct(r.target_margin_bps)),
            ),
        ],
        tables: vec![
            ("Work orders & projects".into(), work),
            (
                "By property".into(),
                rollup_table(&r.by_property, "Property"),
            ),
            (
                "By technician".into(),
                rollup_table(&r.by_technician, "Technician"),
            ),
            (
                "By category".into(),
                rollup_table(&r.by_category, "Category"),
            ),
            ("By month".into(), rollup_table(&r.by_month, "Month")),
        ],
        landscape: true,
        basename: format!("profit-{}", r.from),
    }
}

/// `GET /reports/profit?from&to`.
#[rocket_okapi::openapi(tag = "Back office reports")]
#[get("/reports/profit?<from>&<to>")]
pub async fn profit(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
) -> ApiResult<Json<ProfitReport>> {
    user.require(Permission::PayrollRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (f, t) = period(from.as_deref(), to.as_deref(), &rules)?;
    Ok(Json(profit_data(&db, scope.tenant_id, f, t, &rules).await?))
}

/// `GET /reports/profit/export?from&to&format&section`.
#[rocket_okapi::openapi(skip)]
#[get("/reports/profit/export?<from>&<to>&<format>&<section>")]
pub async fn profit_export(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
    format: Option<String>,
    section: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::PayrollRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (f, t) = period(from.as_deref(), to.as_deref(), &rules)?;
    let r = profit_data(&db, scope.tenant_id, f, t, &rules).await?;
    export(
        &db,
        scope.tenant_id,
        profit_printable(&r),
        format.as_deref().unwrap_or("pdf"),
        section.as_deref(),
    )
    .await
}

// ---------------------------------------------------------------------------
// Tax package
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct KeyDate {
    pub date: String,
    pub what: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PayByPerson {
    pub user_id: Uuid,
    pub name: String,
    /// `W-2` | `1099-NEC`
    pub form: String,
    pub employment: String,
    pub minutes: i64,
    pub overtime_minutes: i64,
    pub double_minutes: i64,
    pub gross_cents: i64,
    pub mileage_paid_back_cents: i64,
    pub miles: f64,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TaxReport {
    pub from: String,
    pub to: String,
    pub label: String,
    pub deductible_expenses_cents: i64,
    pub nondeductible_expenses_cents: i64,
    pub mileage_cents: i64,
    pub miles: f64,
    pub reimbursed_cents: i64,
    pub w2_gross_cents: i64,
    pub contractor_gross_cents: i64,
    pub billed_to_owners_cents: i64,
    pub missing_receipts: usize,
    pub by_category: Vec<(String, i64)>,
    pub pay_by_person: Vec<PayByPerson>,
    pub key_dates: Vec<KeyDate>,
    #[serde(skip)]
    mileage_rows: Vec<Vec<String>>,
    #[serde(skip)]
    expense_rows: Vec<Vec<String>>,
    #[serde(skip)]
    missing_rows: Vec<Vec<String>>,
}

/// General federal + California dates for a property-management business —
/// a reminder list, not advice; the accountant confirms.
pub fn key_dates(year: i32) -> Vec<KeyDate> {
    let d = |y: i32, m: u32, day: u32, what: &str| KeyDate {
        date: format!("{y}-{m:02}-{day:02}"),
        what: what.into(),
    };
    let n = year + 1;
    vec![
        d(year, 1, 31, "W-2s and 1099-NECs (last year) to people and the IRS; 1099-MISC rents to owners; Q4 Form 941 and CA DE 9"),
        d(year, 2, 1, "California property tax, second installment due (delinquent after Apr 10)"),
        d(year, 3, 15, "Partnership (1065) / S-corp (1120-S) returns, or extension; K-1s to partners"),
        d(year, 4, 15, "1040 + Schedule C/E and CA 540; Q1 estimated tax (federal; CA 30%)"),
        d(year, 4, 30, "Q1 Form 941 and CA DE 9 / DE 9C"),
        d(year, 6, 15, "Q2 estimated tax (federal; CA 40%)"),
        d(year, 7, 31, "Q2 Form 941 and CA DE 9 / DE 9C"),
        d(year, 9, 15, "Q3 estimated tax (federal; CA 0%); extended 1065 / 1120-S"),
        d(year, 10, 31, "Q3 Form 941 and CA DE 9 / DE 9C"),
        d(year, 11, 1, "California property tax, first installment due (delinquent after Dec 10)"),
        d(n, 1, 15, "Q4 estimated tax (federal; CA 30%)"),
        d(n, 1, 31, "W-2s and 1099-NECs for this year; 1099-MISC to owners; Q4 941 and DE 9"),
    ]
}

pub(crate) async fn tax_data(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    year: i32,
    quarter: Option<u32>,
    rules: &Rules,
) -> ApiResult<TaxReport> {
    let (from, to, label) = match quarter {
        Some(q @ 1..=4) => {
            let start = NaiveDate::from_ymd_opt(year, (q - 1) * 3 + 1, 1).expect("date");
            let end = if q == 4 {
                NaiveDate::from_ymd_opt(year, 12, 31).expect("date")
            } else {
                NaiveDate::from_ymd_opt(year, q * 3 + 1, 1).expect("date") - Duration::days(1)
            };
            (start, end, format!("Q{q} {year}"))
        }
        Some(_) => return Err(ApiError::BadRequest("quarter must be 1–4".into())),
        None => (
            NaiveDate::from_ymd_opt(year, 1, 1)
                .ok_or_else(|| ApiError::BadRequest("bad year".into()))?,
            NaiveDate::from_ymd_opt(year, 12, 31).expect("date"),
            year.to_string(),
        ),
    };
    let expenses = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(tenant_id))
        .filter(entity::expense::Column::IncurredOn.gte(from.to_string()))
        .filter(entity::expense::Column::IncurredOn.lte(to.to_string()))
        .order_by_asc(entity::expense::Column::IncurredOn)
        .all(db)
        .await?;
    let with_receipt: HashSet<Uuid> = Doc::find()
        .filter(entity::document::Column::TenantId.eq(tenant_id))
        .filter(entity::document::Column::OwnerType.eq("expense"))
        .filter(entity::document::Column::Status.eq("stored"))
        .filter(
            entity::document::Column::OwnerId
                .is_in(expenses.iter().map(|x| x.id).collect::<Vec<_>>()),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|d| d.owner_id)
        .collect();
    let who = names(db, expenses.iter().filter_map(|x| x.user_id)).await?;
    let tickets: HashMap<Uuid, String> = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(
            entity::maintenance_ticket::Column::Id.is_in(
                expenses
                    .iter()
                    .filter_map(|x| x.maintenance_ticket_id)
                    .collect::<Vec<_>>(),
            ),
        )
        .all(db)
        .await?
        .into_iter()
        .map(|t| (t.id, t.title))
        .collect();
    let props: HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let s = |v: &serde_json::Value, k: &str| -> String {
        v.get(k)
            .map(|x| {
                x.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| x.to_string())
            })
            .filter(|x| x != "null")
            .unwrap_or_default()
    };
    let (mut mileage_rows, mut expense_rows, mut missing_rows) =
        (Vec::new(), Vec::new(), Vec::new());
    let mut by_category: BTreeMap<String, i64> = BTreeMap::new();
    let (mut deductible, mut nondeductible, mut mileage, mut hundredths, mut reimbursed) =
        (0, 0, 0, 0, 0);
    for x in &expenses {
        let person = x
            .user_id
            .and_then(|u| who.get(&u).cloned())
            .unwrap_or_default();
        let work = x
            .maintenance_ticket_id
            .and_then(|t| tickets.get(&t).cloned())
            .unwrap_or_default();
        let property = x
            .property_id
            .and_then(|p| props.get(&p).cloned())
            .unwrap_or_default();
        if x.reimbursed_at.is_some() {
            reimbursed += x.amount_cents;
        }
        if x.category == "mileage" {
            mileage += x.amount_cents;
            hundredths += x.miles_hundredths.unwrap_or(0);
            mileage_rows.push(vec![
                x.incurred_on.clone(),
                person,
                if x.description.is_empty() {
                    s(&x.details, "purpose")
                } else {
                    x.description.clone()
                },
                s(&x.details, "from"),
                s(&x.details, "to"),
                s(&x.details, "odometer_start"),
                s(&x.details, "odometer_end"),
                format!("{:.1}", x.miles_hundredths.unwrap_or(0) as f64 / 100.0),
                x.vehicle.clone(),
                format!("${:.3}", x.mileage_rate_mills.unwrap_or(0) as f64 / 1000.0),
                money(x.amount_cents),
                if x.reimbursable {
                    if x.reimbursed_at.is_some() {
                        "paid back".into()
                    } else {
                        "owed".into()
                    }
                } else {
                    String::new()
                },
                work,
                property,
            ]);
            continue;
        }
        if x.tax_deductible {
            deductible += x.amount_cents;
        } else {
            nondeductible += x.amount_cents;
        }
        *by_category.entry(x.category.clone()).or_default() += x.amount_cents;
        let has = with_receipt.contains(&x.id);
        expense_rows.push(vec![
            x.incurred_on.clone(),
            x.category.clone(),
            x.vendor.clone().unwrap_or_default(),
            x.description.clone(),
            money(x.amount_cents),
            if x.tax_deductible {
                "yes".into()
            } else {
                "no".into()
            },
            if has { "yes".into() } else { "MISSING".into() },
            work.clone(),
            property.clone(),
            person.clone(),
        ]);
        if x.tax_deductible && !has {
            missing_rows.push(vec![
                x.incurred_on.clone(),
                x.category.clone(),
                x.vendor.clone().unwrap_or_default(),
                money(x.amount_cents),
                person,
            ]);
        }
    }
    // Pay by person, over the whole weeks that start in the period.
    let entries = workforce::entries_for_weeks(db, tenant_id, from, to, None, rules).await?;
    let profiles = workforce::profiles_by_user(db, tenant_id).await?;
    let (weeks, _) = workforce::week_splits(&entries, &profiles, rules);
    let mut pay: BTreeMap<Uuid, PayByPerson> = BTreeMap::new();
    let pw = names(db, weeks.iter().map(|w| w.user_id)).await?;
    for w in weeks
        .iter()
        .filter(|w| w.monday >= overtime::week_start(from) && w.monday <= to)
    {
        let prof = profiles.get(&w.user_id);
        let contractor = prof.is_some_and(|p| p.employment_type == "contractor");
        let row = pay.entry(w.user_id).or_insert_with(|| PayByPerson {
            user_id: w.user_id,
            name: pw.get(&w.user_id).cloned().unwrap_or_default(),
            form: if contractor {
                "1099-NEC".into()
            } else {
                "W-2".into()
            },
            employment: employment_label(
                prof.map(|p| p.employment_type.as_str())
                    .unwrap_or("full_time"),
            )
            .into(),
            minutes: 0,
            overtime_minutes: 0,
            double_minutes: 0,
            gross_cents: 0,
            mileage_paid_back_cents: 0,
            miles: 0.0,
        });
        row.minutes += w.split.minutes();
        row.overtime_minutes += w.split.overtime;
        row.double_minutes += w.split.double;
        row.gross_cents += w.gross_cents;
    }
    for x in expenses
        .iter()
        .filter(|x| x.category == "mileage" && x.reimbursable)
    {
        if let Some(r) = x.user_id.and_then(|u| pay.get_mut(&u)) {
            r.mileage_paid_back_cents += x.amount_cents;
            r.miles += x.miles_hundredths.unwrap_or(0) as f64 / 100.0;
        }
    }
    let pay_by_person: Vec<PayByPerson> = pay.into_values().collect();
    // In-house work billed to owners in the period (live in-house bills).
    let in_house = costing::in_house_vendor_ids(db, tenant_id).await?;
    let billed: i64 = entity::prelude::VendorBill::find()
        .filter(entity::vendor_bill::Column::TenantId.eq(tenant_id))
        .filter(
            entity::vendor_bill::Column::CounterpartyId
                .is_in(in_house.into_iter().collect::<Vec<_>>()),
        )
        .filter(entity::vendor_bill::Column::CreatedAt.gte(rules.day_start(from)))
        .filter(entity::vendor_bill::Column::CreatedAt.lt(rules.day_start(to + Duration::days(1))))
        .all(db)
        .await?
        .into_iter()
        .filter(costing::bill_counts)
        .map(|b| b.amount_cents)
        .sum();
    Ok(TaxReport {
        from: from.to_string(),
        to: to.to_string(),
        label,
        deductible_expenses_cents: deductible,
        nondeductible_expenses_cents: nondeductible,
        mileage_cents: mileage,
        miles: hundredths as f64 / 100.0,
        reimbursed_cents: reimbursed,
        w2_gross_cents: pay_by_person
            .iter()
            .filter(|p| p.form == "W-2")
            .map(|p| p.gross_cents)
            .sum(),
        contractor_gross_cents: pay_by_person
            .iter()
            .filter(|p| p.form != "W-2")
            .map(|p| p.gross_cents)
            .sum(),
        billed_to_owners_cents: billed,
        missing_receipts: missing_rows.len(),
        by_category: by_category.into_iter().collect(),
        pay_by_person,
        key_dates: key_dates(year),
        mileage_rows,
        expense_rows,
        missing_rows,
    })
}

fn tax_printable(r: TaxReport) -> Printable {
    let simple =
        |cols: &[(&str, bool, f64)], rows: Vec<Vec<String>>, totals: Option<Vec<String>>| Table {
            columns: cols
                .iter()
                .map(|(l, right, w)| {
                    if *right {
                        Column::right(l, *w)
                    } else {
                        Column::left(l, *w)
                    }
                })
                .collect(),
            rows,
            totals,
        };
    let pay_rows = r
        .pay_by_person
        .iter()
        .map(|p| {
            vec![
                p.name.clone(),
                p.form.clone(),
                p.employment.clone(),
                hm(p.minutes),
                hm(p.overtime_minutes),
                hm(p.double_minutes),
                money(p.gross_cents),
                money(p.mileage_paid_back_cents),
                format!("{:.1}", p.miles),
            ]
        })
        .collect();
    Printable {
        title: format!("Tax package — {}", r.label),
        subtitle: Some(format!(
            "{} – {} · a working file for your accountant, not tax advice",
            r.from, r.to
        )),
        summary: vec![
            (
                "Deductible expenses".into(),
                money(r.deductible_expenses_cents),
            ),
            (
                "Not deductible".into(),
                money(r.nondeductible_expenses_cents),
            ),
            (
                "Mileage".into(),
                format!("{} ({:.1} miles)", money(r.mileage_cents), r.miles),
            ),
            ("Paid back to people".into(), money(r.reimbursed_cents)),
            ("W-2 gross pay".into(), money(r.w2_gross_cents)),
            (
                "1099 contractor pay".into(),
                money(r.contractor_gross_cents),
            ),
            (
                "In-house work billed to owners".into(),
                money(r.billed_to_owners_cents),
            ),
            (
                "Deductible expenses missing a receipt".into(),
                r.missing_receipts.to_string(),
            ),
        ],
        tables: vec![
            (
                "Expenses by category".into(),
                simple(
                    &[("Category", false, 3.0), ("Amount", true, 1.0)],
                    r.by_category
                        .iter()
                        .map(|(c, a)| vec![c.clone(), money(*a)])
                        .collect(),
                    Some(vec![
                        "Total".into(),
                        money(r.by_category.iter().map(|(_, a)| a).sum()),
                    ]),
                ),
            ),
            (
                "Pay by person".into(),
                simple(
                    &[
                        ("Name", false, 14.0),
                        ("Form", false, 7.0),
                        ("Employment", false, 11.0),
                        ("Hours", true, 7.0),
                        ("OT", true, 6.0),
                        ("DT", true, 6.0),
                        ("Gross", true, 10.0),
                        ("Mileage back", true, 10.0),
                        ("Miles", true, 6.0),
                    ],
                    pay_rows,
                    None,
                ),
            ),
            (
                "Mileage log".into(),
                simple(
                    &[
                        ("Date", false, 8.0),
                        ("Driver", false, 10.0),
                        ("Purpose", false, 14.0),
                        ("From", false, 10.0),
                        ("To", false, 10.0),
                        ("Odo start", true, 7.0),
                        ("Odo end", true, 7.0),
                        ("Miles", true, 6.0),
                        ("Vehicle", false, 7.0),
                        ("Rate", true, 6.0),
                        ("Amount", true, 8.0),
                        ("Paid back", false, 7.0),
                        ("Work order", false, 12.0),
                        ("Property", false, 10.0),
                    ],
                    r.mileage_rows,
                    Some(vec![
                        "Total".into(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        String::new(),
                        format!("{:.1}", r.miles),
                        String::new(),
                        String::new(),
                        money(r.mileage_cents),
                        String::new(),
                        String::new(),
                        String::new(),
                    ]),
                ),
            ),
            (
                "Expense ledger".into(),
                simple(
                    &[
                        ("Date", false, 8.0),
                        ("Category", false, 8.0),
                        ("Vendor", false, 11.0),
                        ("Description", false, 16.0),
                        ("Amount", true, 8.0),
                        ("Deductible", false, 6.0),
                        ("Receipt", false, 6.0),
                        ("Work order", false, 12.0),
                        ("Property", false, 10.0),
                        ("Who", false, 9.0),
                    ],
                    r.expense_rows,
                    None,
                ),
            ),
            (
                "Missing receipts".into(),
                simple(
                    &[
                        ("Date", false, 8.0),
                        ("Category", false, 8.0),
                        ("Vendor", false, 14.0),
                        ("Amount", true, 8.0),
                        ("Who", false, 10.0),
                    ],
                    r.missing_rows,
                    None,
                ),
            ),
            (
                "Key dates".into(),
                simple(
                    &[("Date", false, 1.0), ("What", false, 7.0)],
                    r.key_dates
                        .iter()
                        .map(|k| vec![k.date.clone(), k.what.clone()])
                        .collect(),
                    None,
                ),
            ),
        ],
        landscape: true,
        basename: format!("tax-package-{}", slug(&r.label)),
    }
}

fn year_default(rules: &Rules, year: Option<i32>) -> i32 {
    year.unwrap_or_else(|| rules.local_date(Utc::now().into()).year())
}

/// `GET /reports/taxes?year&quarter`.
#[rocket_okapi::openapi(tag = "Back office reports")]
#[get("/reports/taxes?<year>&<quarter>")]
pub async fn taxes(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    year: Option<i32>,
    quarter: Option<u32>,
) -> ApiResult<Json<TaxReport>> {
    user.require(Permission::PayrollRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(
        tax_data(
            &db,
            scope.tenant_id,
            year_default(&rules, year),
            quarter,
            &rules,
        )
        .await?,
    ))
}

/// `GET /reports/taxes/export?year&quarter&format&section` — the whole package
/// as one PDF, or one section (`mileage-log`, `expense-ledger`,
/// `missing-receipts`, `pay-by-person`, …) as CSV.
#[rocket_okapi::openapi(skip)]
#[get("/reports/taxes/export?<year>&<quarter>&<format>&<section>")]
pub async fn taxes_export(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    year: Option<i32>,
    quarter: Option<u32>,
    format: Option<String>,
    section: Option<String>,
) -> ApiResult<ReportFile> {
    user.require(Permission::PayrollRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let r = tax_data(
        &db,
        scope.tenant_id,
        year_default(&rules, year),
        quarter,
        &rules,
    )
    .await?;
    export(
        &db,
        scope.tenant_id,
        tax_printable(r),
        format.as_deref().unwrap_or("pdf"),
        section.as_deref(),
    )
    .await
}

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct Dashboard {
    pub clocked_in: Vec<String>,
    pub team_size: usize,
    pub hours_this_week_minutes: i64,
    pub unapproved_entries: usize,
    pub missed_punches: usize,
    pub pending_time_off: usize,
    pub expenses_to_reimburse: usize,
    pub expenses_to_reimburse_cents: i64,
    /// Approved in-house time not billed to an owner yet, at bill rates.
    pub unbilled_time_cents: i64,
    pub work_orders_with_unbilled_time: usize,
    /// Only with `payroll:read`.
    pub labor_cost_this_week_cents: Option<i64>,
    pub billed_to_owners_this_month_cents: i64,
}

/// `GET /backoffice/dashboard`.
#[rocket_okapi::openapi(tag = "Back office reports")]
#[get("/backoffice/dashboard")]
pub async fn dashboard(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Dashboard>> {
    user.require(Permission::TeamRead)?;
    let t = scope.tenant_id;
    workforce::sweep_missed_punches(&db, t, Utc::now()).await?;
    let rules = Rules::load(&db, t).await;
    let now = Utc::now();
    let today = rules.local_date(now.into());
    let monday = overtime::week_start(today);
    let week = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(t))
        .filter(entity::time_entry::Column::StartedAt.gte(rules.day_start(monday)))
        .all(&db)
        .await?;
    let open = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(t))
        .filter(entity::time_entry::Column::EndedAt.is_null())
        .all(&db)
        .await?;
    let unapproved = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(t))
        .filter(entity::time_entry::Column::EndedAt.is_not_null())
        .filter(entity::time_entry::Column::ApprovedAt.is_null())
        .all(&db)
        .await?;
    let who = names(&db, open.iter().map(|e| e.user_id)).await?;
    let profiles = EmployeeProfile::find()
        .filter(entity::employee_profile::Column::TenantId.eq(t))
        .all(&db)
        .await?;
    let pay: HashMap<Uuid, i64> = profiles
        .iter()
        .map(|p| (p.user_id, p.pay_rate_cents))
        .collect();
    let reimb = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(t))
        .filter(entity::expense::Column::Reimbursable.eq(true))
        .filter(entity::expense::Column::ReimbursedAt.is_null())
        .all(&db)
        .await?;
    // Unbilled approved time, and the work orders it sits on.
    let approved_unbilled = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(t))
        .filter(entity::time_entry::Column::ApprovedAt.is_not_null())
        .filter(entity::time_entry::Column::MaintenanceTicketId.is_not_null())
        .all(&db)
        .await?;
    let live = crate::routes::team::billed_live(&db, t, &approved_unbilled).await?;
    let unbilled: Vec<&entity::time_entry::Model> = approved_unbilled
        .iter()
        .filter(|e| !e.billed_bill_id.is_some_and(|b| live.contains(&b)))
        .collect();
    let in_house = costing::in_house_vendor_ids(&db, t).await?;
    let month_start = rules.day_start(today.with_day(1).expect("day 1"));
    let billed_month: i64 = entity::prelude::VendorBill::find()
        .filter(entity::vendor_bill::Column::TenantId.eq(t))
        .filter(
            entity::vendor_bill::Column::CounterpartyId
                .is_in(in_house.into_iter().collect::<Vec<_>>()),
        )
        .filter(entity::vendor_bill::Column::CreatedAt.gte(month_start))
        .all(&db)
        .await?
        .into_iter()
        .filter(costing::bill_counts)
        .map(|b| b.amount_cents)
        .sum();
    Ok(Json(Dashboard {
        clocked_in: open
            .iter()
            .filter_map(|e| who.get(&e.user_id).cloned())
            .collect(),
        team_size: profiles
            .iter()
            .filter(|p| workforce::is_current(p, today))
            .count(),
        hours_this_week_minutes: week.iter().map(|e| entry_minutes(e, now)).sum(),
        unapproved_entries: unapproved.len(),
        missed_punches: unapproved.iter().filter(|e| needs_review(e)).count(),
        pending_time_off: TimeOffRequest::find()
            .filter(entity::time_off_request::Column::TenantId.eq(t))
            .filter(entity::time_off_request::Column::Status.eq("pending"))
            .all(&db)
            .await?
            .len(),
        expenses_to_reimburse: reimb.len(),
        expenses_to_reimburse_cents: reimb.iter().map(|x| x.amount_cents).sum(),
        unbilled_time_cents: unbilled
            .iter()
            .map(|e| minutes_cost(entry_minutes(e, now), e.bill_rate_cents.unwrap_or(0)))
            .sum(),
        work_orders_with_unbilled_time: unbilled
            .iter()
            .filter_map(|e| e.maintenance_ticket_id)
            .collect::<HashSet<_>>()
            .len(),
        labor_cost_this_week_cents: sees_pay(&user).then(|| {
            week.iter()
                .map(|e| {
                    minutes_cost(
                        entry_minutes(e, now),
                        e.pay_rate_cents
                            .or(pay.get(&e.user_id).copied())
                            .unwrap_or(0),
                    )
                })
                .sum()
        }),
        billed_to_owners_this_month_cents: billed_month,
    }))
}

// ---------------------------------------------------------------------------
// Cost sheet (one work order / project, printable)
// ---------------------------------------------------------------------------

/// `GET /costs/<kind>/<id>/sheet.pdf` — the work's owner-bill preview, its
/// time, and (with `payroll:read`) the full cost breakdown, as one PDF.
#[rocket_okapi::openapi(skip)]
#[get("/costs/<kind>/<id>/sheet.pdf")]
pub async fn cost_sheet(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    kind: &str,
    id: &str,
) -> ApiResult<ReportFile> {
    user.require(Permission::TeamRead)?;
    let t = scope.tenant_id;
    let (work, property_id, title) = resolve_work(&db, t, kind, id).await?;
    let rules = Rules::load(&db, t).await;
    let pv = preview(&db, t, work, property_id, title.clone(), &rules).await?;
    let property = Property::find_by_id(property_id)
        .one(&db)
        .await?
        .map(|p| format!("{} · {}", p.name, p.address))
        .unwrap_or_default();
    let entries = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(t))
        .filter(match work {
            Work::Ticket(x) => entity::time_entry::Column::MaintenanceTicketId.eq(x),
            Work::Project(x) => entity::time_entry::Column::RehabProjectId.eq(x),
        })
        .order_by_asc(entity::time_entry::Column::StartedAt)
        .all(&db)
        .await?;
    let dtos = crate::routes::team::entry_dtos(&db, t, entries, false, &rules).await?;
    let when = |s: &str| {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|d| {
                d.with_timezone(&rules.tz)
                    .format("%b %-d, %-I:%M %p")
                    .to_string()
            })
            .unwrap_or_default()
    };
    let mut blocks = vec![
        Block::KeyValues(vec![("Property".into(), property)]),
        Block::Heading("To bill the owner".into()),
        Block::Table(Table {
            columns: vec![Column::left("Line", 5.0), Column::right("Amount", 1.0)],
            rows: pv
                .lines
                .iter()
                .map(|l| vec![l.description.clone(), money(l.amount_cents)])
                .collect(),
            totals: Some(vec!["Total".into(), money(pv.total_cents)]),
        }),
    ];
    if !pv.held_back.is_empty() {
        blocks.push(Block::Paragraph(format!(
            "Not included yet: {}.",
            pv.held_back.join("; ")
        )));
    }
    if !pv.previous_bills.is_empty() {
        blocks.push(Block::Paragraph(format!(
            "Already billed: {}.",
            pv.previous_bills
                .iter()
                .map(|b| format!("{} {} ({})", b.bill_number, money(b.amount_cents), b.status))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    blocks.push(Block::Heading("Time".into()));
    blocks.push(Block::Table(Table {
        columns: vec![
            Column::left("Who", 4.0),
            Column::left("Started", 4.0),
            Column::left("Ended", 4.0),
            Column::right("Hours", 2.0),
            Column::left("Status", 3.0),
        ],
        rows: dtos
            .iter()
            .map(|d| {
                vec![
                    d.user_name.clone(),
                    when(&d.started_at),
                    d.ended_at
                        .as_deref()
                        .map(when)
                        .unwrap_or_else(|| "on the clock".into()),
                    hm(d.minutes),
                    if d.billed {
                        "billed".into()
                    } else if d.approved {
                        "approved".into()
                    } else {
                        "waiting".into()
                    },
                ]
            })
            .collect(),
        totals: Some(vec![
            "Total".into(),
            String::new(),
            String::new(),
            hm(dtos.iter().map(|d| d.minutes).sum()),
            String::new(),
        ]),
    }));
    if sees_pay(&user) {
        let c = costing::cost_work(&db, t, &[work], &rules)
            .await?
            .remove(&work)
            .unwrap_or_default();
        blocks.push(Block::Heading("Cost & margin".into()));
        blocks.push(Block::KeyValues(vec![
            ("Labor pay".into(), money(c.labor_pay_cents)),
            (
                "Overtime premium share".into(),
                money(c.overtime_premium_cents),
            ),
            ("Labor burden".into(), money(c.burden_cents)),
            (
                "Parts & line items".into(),
                money(c.parts_cents + c.other_lines_cents),
            ),
            ("Mileage".into(), money(c.mileage_cents)),
            ("Other expenses".into(), money(c.expenses_cents)),
            ("Total cost".into(), money(c.costs_cents)),
            (
                "Billed / unbilled".into(),
                format!("{} / {}", money(c.billed_cents), money(c.unbilled_cents)),
            ),
            (
                "Gross".into(),
                format!("{} ({})", money(c.gross_cents), pct(c.gross_bps)),
            ),
            ("Net after overhead".into(), money(c.net_cents)),
            (
                "Outside vendor bills (owner's cost)".into(),
                money(c.vendor_bills_cents),
            ),
            ("Total to the owner".into(), money(c.owner_total_cents)),
            (
                "Bill rate for the target margin".into(),
                format!(
                    "{}/h (target {})",
                    money(c.bill_rate_for_target_cents),
                    pct(rules.target_margin_bps)
                ),
            ),
        ]));
    }
    let doc = Document {
        title: format!("Cost sheet — {title}"),
        subtitle: Some(format!(
            "Printed {}",
            rules.local_date(Utc::now().into()).format("%b %-d, %Y")
        )),
        organization: org_name(&db, t).await,
        landscape: false,
        blocks,
    };
    Ok(ReportFile::new(
        crate::pdfdoc::render(&doc),
        "application/pdf",
        format!("cost-sheet-{}.pdf", slug(&title)),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("Mileage log"), "mileage-log");
        assert_eq!(slug("By technician"), "by-technician");
        assert_eq!(slug("Q3 2026"), "q3-2026");
    }

    #[test]
    fn key_dates_cover_the_year_and_next_january() {
        let k = key_dates(2026);
        assert!(k.iter().any(|d| d.date == "2026-04-15"));
        assert!(k.iter().any(|d| d.date == "2027-01-31"));
        assert!(k.windows(2).all(|w| w[0].date <= w[1].date));
    }
}
