//! **The back office** (Vantedge phase 2B) — people, the clock, and the money
//! their time and spending turn into.
//!
//! Everything here reads from the same rows, the way Alpha Power Wash's back
//! office does: a [`entity::time_entry`] is logged *against* a work order,
//! rehab project or property, and that one row is hours worked, payroll
//! (split by [`overtime`]), labor cost with its share of the week's overtime
//! premium ([`costing`]), and in-house work billed to the owner.
//!
//! * [`Rules`] — the workspace's back-office settings, read once per request.
//! * The clock: [`clock_in`], [`close_entry`], [`find_overlap`], and the
//!   missed-punch sweeper [`sweep_missed_punches`].
//! * [`week_splits`] — each person-week split by the overtime rule, plus each
//!   entry's share of that week's premium.

pub mod costing;
pub mod overtime;

use crate::error::{ApiError, ApiResult};
use crate::settings;
use chrono::{DateTime, Duration, FixedOffset, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use entity::prelude::{
    EmployeeProfile, MaintenanceTicket, PropertyDetail, RehabProject, TimeEntry,
};
use overtime::{Rule, Split};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DbErr, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

/// What a time entry can be logged against.
pub const ENTRY_KINDS: &[&str] = &[
    "work_order",
    "project",
    "property",
    "travel",
    "shop",
    "admin",
    "other",
];

/// Employment types; `contractor` is 1099 (straight time, no burden).
pub const EMPLOYMENT_TYPES: &[&str] = &["full_time", "part_time", "seasonal", "contractor"];

/// Expense categories (Alpha's, with property-management wording).
pub const EXPENSE_CATEGORIES: &[&str] = &[
    "fuel",
    "mileage",
    "materials",
    "equipment",
    "repairs",
    "vehicle",
    "insurance",
    "payroll",
    "marketing",
    "software",
    "licenses",
    "other",
];

/// The workspace's back-office settings.
#[derive(Debug, Clone)]
pub struct Rules {
    pub tz: Tz,
    pub overtime: Rule,
    pub burden_bps: i64,
    pub overhead_per_hour_cents: i64,
    pub target_margin_bps: i64,
    pub mileage_rate_mills: i64,
    pub markup_bps: i64,
    pub missed_punch_hours: i64,
    pub clock_location: bool,
    pub clock_radius_m: i64,
}

impl Rules {
    pub async fn load(db: &impl ConnectionTrait, tenant_id: Uuid) -> Rules {
        let tz = settings::get_string(db, tenant_id, settings::WORKFORCE_TIMEZONE)
            .await
            .parse::<Tz>()
            .unwrap_or(chrono_tz::America::Los_Angeles);
        Rules {
            tz,
            overtime: Rule::parse(
                &settings::get_string(db, tenant_id, settings::WORKFORCE_OVERTIME_RULE).await,
            ),
            burden_bps: settings::get_i64(db, tenant_id, settings::WORKFORCE_LABOR_BURDEN_BPS)
                .await
                .max(0),
            overhead_per_hour_cents: settings::get_i64(
                db,
                tenant_id,
                settings::WORKFORCE_OVERHEAD_PER_HOUR_CENTS,
            )
            .await
            .max(0),
            target_margin_bps: settings::get_i64(
                db,
                tenant_id,
                settings::WORKFORCE_TARGET_MARGIN_BPS,
            )
            .await
            .clamp(0, 9_900),
            mileage_rate_mills: settings::get_i64(
                db,
                tenant_id,
                settings::WORKFORCE_MILEAGE_RATE_MILLS,
            )
            .await
            .max(0),
            markup_bps: settings::get_i64(
                db,
                tenant_id,
                settings::WORKFORCE_MAINTENANCE_MARKUP_BPS,
            )
            .await
            .max(0),
            missed_punch_hours: settings::get_i64(
                db,
                tenant_id,
                settings::WORKFORCE_MISSED_PUNCH_HOURS,
            )
            .await
            .clamp(4, 24),
            clock_location: settings::get_bool(db, tenant_id, settings::WORKFORCE_CLOCK_LOCATION)
                .await,
            clock_radius_m: settings::get_i64(db, tenant_id, settings::WORKFORCE_CLOCK_RADIUS_M)
                .await
                .clamp(50, 5_000),
        }
    }

    /// The local calendar day an instant falls on.
    pub fn local_date(&self, at: DateTime<FixedOffset>) -> NaiveDate {
        at.with_timezone(&self.tz).date_naive()
    }

    /// Local midnight at the start of `day`, as an instant.
    pub fn day_start(&self, day: NaiveDate) -> DateTime<Utc> {
        let naive = day.and_hms_opt(0, 0, 0).expect("midnight exists");
        self.tz
            .from_local_datetime(&naive)
            .earliest()
            .map(|d| d.with_timezone(&Utc))
            // A DST gap at midnight: fall back to the UTC reading.
            .unwrap_or_else(|| Utc.from_utc_datetime(&naive))
    }
}

/// Minutes worked by an entry: elapsed less breaks, never negative. An open
/// entry counts up to `now`.
pub fn entry_minutes(e: &entity::time_entry::Model, now: DateTime<Utc>) -> i64 {
    let end = e.ended_at.map(|d| d.with_timezone(&Utc)).unwrap_or(now);
    let start = e.started_at.with_timezone(&Utc);
    ((end - start).num_seconds() / 60 - e.break_minutes as i64).max(0)
}

/// Money for `minutes` at an hourly rate in cents, rounded once.
pub fn minutes_cost(minutes: i64, rate_cents: i64) -> i64 {
    overtime::div_round(minutes * rate_cents, 60)
}

/// A missed punch the office hasn't settled: held from payroll and approval.
pub fn needs_review(e: &entity::time_entry::Model) -> bool {
    e.missed_punch && e.resolved_at.is_none()
}

/// The profile for `user_id` in the workspace, if they're on the team.
pub async fn profile(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Option<entity::employee_profile::Model>, DbErr> {
    EmployeeProfile::find()
        .filter(entity::employee_profile::Column::TenantId.eq(tenant_id))
        .filter(entity::employee_profile::Column::UserId.eq(user_id))
        .one(db)
        .await
}

/// Whether a profile is current: no end date in the past.
pub fn is_current(p: &entity::employee_profile::Model, today: NaiveDate) -> bool {
    match p
        .end_date
        .as_deref()
        .and_then(|d| d.parse::<NaiveDate>().ok())
    {
        Some(end) => end >= today,
        None => true,
    }
}

/// What an entry is logged against, resolved and checked against the workspace.
#[derive(Debug, Clone, Default)]
pub struct Target {
    pub kind: String,
    pub maintenance_ticket_id: Option<Uuid>,
    pub rehab_project_id: Option<Uuid>,
    pub property_id: Option<Uuid>,
}

/// Validate a target: the kind is known, the work order / project exists in the
/// workspace, and the property is filled in from it.
pub async fn resolve_target(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    kind: &str,
    ticket: Option<Uuid>,
    project: Option<Uuid>,
    property: Option<Uuid>,
) -> ApiResult<Target> {
    let kind = kind.trim().to_lowercase();
    if !ENTRY_KINDS.contains(&kind.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "kind must be one of {}",
            ENTRY_KINDS.join(", ")
        )));
    }
    let mut t = Target {
        kind: kind.clone(),
        ..Default::default()
    };
    match kind.as_str() {
        "work_order" => {
            let id = ticket.ok_or_else(|| {
                ApiError::BadRequest("pick the work order this time is for".into())
            })?;
            let tk = MaintenanceTicket::find_by_id(id)
                .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
                .ok_or_else(|| ApiError::NotFound("work order not found".into()))?;
            t.maintenance_ticket_id = Some(tk.id);
            t.property_id = Some(tk.property_id);
        }
        "project" => {
            let id =
                project.ok_or_else(|| ApiError::BadRequest("pick the rehab project".into()))?;
            let p = RehabProject::find_by_id(id)
                .filter(entity::rehab_project::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
                .ok_or_else(|| ApiError::NotFound("rehab project not found".into()))?;
            t.rehab_project_id = Some(p.id);
            t.property_id = Some(p.property_id);
        }
        "property" => {
            let id = property.ok_or_else(|| ApiError::BadRequest("pick the property".into()))?;
            let p = entity::prelude::Property::find_by_id(id)
                .filter(entity::property::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?
                .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
            t.property_id = Some(p.id);
        }
        // travel / shop / admin / other may still name a property.
        _ => {
            if let Some(id) = property {
                let ok = entity::prelude::Property::find_by_id(id)
                    .filter(entity::property::Column::TenantId.eq(tenant_id))
                    .one(db)
                    .await?
                    .is_some();
                if ok {
                    t.property_id = Some(id);
                }
            }
        }
    }
    Ok(t)
}

/// Another entry of the same person overlapping `[start, end)` (an open one
/// counts as running until now) — nobody is paid twice for the same minutes.
pub async fn find_overlap(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
    start: DateTime<Utc>,
    end: Option<DateTime<Utc>>,
    exclude: Option<Uuid>,
) -> Result<Option<entity::time_entry::Model>, DbErr> {
    let end = end.unwrap_or_else(Utc::now);
    let mut q = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::UserId.eq(user_id))
        .filter(entity::time_entry::Column::StartedAt.lt(end))
        .filter(
            Condition::any()
                .add(entity::time_entry::Column::EndedAt.is_null())
                .add(entity::time_entry::Column::EndedAt.gt(start)),
        );
    if let Some(x) = exclude {
        q = q.filter(entity::time_entry::Column::Id.ne(x));
    }
    q.order_by_asc(entity::time_entry::Column::StartedAt)
        .one(db)
        .await
}

/// The person's open entry, if they're clocked in.
pub async fn open_entry(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<Option<entity::time_entry::Model>, DbErr> {
    TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::UserId.eq(user_id))
        .filter(entity::time_entry::Column::EndedAt.is_null())
        .one(db)
        .await
}

/// A punch location: where the phone was, and how far from the property.
#[derive(Debug, Clone, Copy)]
pub struct Fix {
    pub lat: f64,
    pub lng: f64,
}

/// Great-circle distance in metres.
pub fn haversine_m(a: Fix, b: Fix) -> f64 {
    let r = 6_371_000.0_f64;
    let (la1, la2) = (a.lat.to_radians(), b.lat.to_radians());
    let dla = (b.lat - a.lat).to_radians();
    let dlo = (b.lng - a.lng).to_radians();
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    2.0 * r * h.sqrt().asin()
}

/// Distance from the property to a fix, when the property has coordinates.
pub async fn distance_from_property(
    db: &impl ConnectionTrait,
    property_id: Option<Uuid>,
    fix: Option<Fix>,
) -> Option<i32> {
    let (pid, fix) = (property_id?, fix?);
    let d = PropertyDetail::find_by_id(pid).one(db).await.ok()??;
    let here = Fix {
        lat: d.latitude?,
        lng: d.longitude?,
    };
    Some(haversine_m(here, fix).round() as i32)
}

/// Clock `user_id` in against `target`, closing any entry they left open
/// (starting new work ends the old, as in Alpha).
pub async fn clock_in(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
    target: Target,
    notes: Option<String>,
    fix: Option<Fix>,
    rules: &Rules,
) -> ApiResult<entity::time_entry::Model> {
    let now = Utc::now();
    if let Some(open) = open_entry(db, tenant_id, user_id).await? {
        close_entry(db, tenant_id, open, now, None, rules).await?;
    }
    let fix = fix.filter(|_| rules.clock_location);
    let distance = distance_from_property(db, target.property_id, fix).await;
    Ok(entity::time_entry::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        user_id: Set(user_id),
        kind: Set(target.kind),
        maintenance_ticket_id: Set(target.maintenance_ticket_id),
        rehab_project_id: Set(target.rehab_project_id),
        property_id: Set(target.property_id),
        started_at: Set(now.into()),
        ended_at: Set(None),
        break_minutes: Set(0),
        notes: Set(notes),
        pay_rate_cents: Set(None),
        bill_rate_cents: Set(None),
        approved_by: Set(None),
        approved_at: Set(None),
        missed_punch: Set(false),
        missed_punch_reason: Set(None),
        claimed_end: Set(None),
        punch_note: Set(None),
        resolved_by: Set(None),
        resolved_at: Set(None),
        in_lat: Set(fix.map(|f| f.lat)),
        in_lng: Set(fix.map(|f| f.lng)),
        in_distance_m: Set(distance),
        out_lat: Set(None),
        out_lng: Set(None),
        out_distance_m: Set(None),
        billed_bill_id: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?)
}

/// Close an entry at `at`, freezing the pay and bill rates that applied — a
/// raise next month never rewrites last month's payroll or owner bills.
pub async fn close_entry(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    entry: entity::time_entry::Model,
    at: DateTime<Utc>,
    fix: Option<Fix>,
    rules: &Rules,
) -> ApiResult<entity::time_entry::Model> {
    if entry.ended_at.is_some() {
        return Err(ApiError::Conflict("this entry is already closed".into()));
    }
    let start = entry.started_at.with_timezone(&Utc);
    let at = if at <= start {
        start + Duration::minutes(1)
    } else {
        at
    };
    let prof = profile(db, tenant_id, entry.user_id).await?;
    let fix = fix.filter(|_| rules.clock_location);
    let distance = distance_from_property(db, entry.property_id, fix).await;
    let mut am: entity::time_entry::ActiveModel = entry.into();
    am.ended_at = Set(Some(at.into()));
    am.pay_rate_cents = Set(Some(prof.as_ref().map(|p| p.pay_rate_cents).unwrap_or(0)));
    am.bill_rate_cents = Set(Some(prof.as_ref().map(|p| p.bill_rate_cents).unwrap_or(0)));
    if fix.is_some() {
        am.out_lat = Set(fix.map(|f| f.lat));
        am.out_lng = Set(fix.map(|f| f.lng));
        am.out_distance_m = Set(distance);
    }
    am.updated_at = Set(Utc::now().into());
    Ok(am.update(db).await?)
}

/// Close every clock-in left running too long (or whose work order was resolved
/// over an hour ago) at the best evidence, flag it as a missed punch, and tell
/// the person. Returns how many were closed.
pub async fn sweep_missed_punches(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    now: DateTime<Utc>,
) -> ApiResult<usize> {
    let rules = Rules::load(db, tenant_id).await;
    let open = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::EndedAt.is_null())
        .all(db)
        .await?;
    let mut closed = 0;
    for e in open {
        let start = e.started_at.with_timezone(&Utc);
        let limit = start + Duration::hours(rules.missed_punch_hours);
        let ticket = match e.maintenance_ticket_id {
            Some(id) => MaintenanceTicket::find_by_id(id).one(db).await?,
            None => None,
        };
        let resolved = ticket
            .as_ref()
            .and_then(|t| t.resolved_at)
            .map(|d| d.with_timezone(&Utc))
            .filter(|r| *r > start);
        let job_done = resolved.is_some_and(|r| now - r > Duration::hours(1));
        if now < limit && !job_done {
            continue;
        }
        let (end, why) = match resolved.filter(|r| *r <= limit.min(now)) {
            Some(r) => (r, "the work order was marked resolved then"),
            None => (
                (start + Duration::hours(8)).min(now),
                "nobody clocked out — closed at 8 hours",
            ),
        };
        let user_id = e.user_id;
        let entry = close_entry(db, tenant_id, e, end, None, &rules).await?;
        let mut am: entity::time_entry::ActiveModel = entry.clone().into();
        am.missed_punch = Set(true);
        am.missed_punch_reason = Set(Some(why.to_string()));
        am.update(db).await?;
        crate::audit::record(
            db,
            None,
            crate::audit::actions::TIME_MISSED_PUNCH,
            Some("time_entry"),
            Some(entry.id.to_string()),
            Some(tenant_id),
            Some(serde_json::json!({ "reason": why })),
        )
        .await;
        if let Ok(Some(u)) = entity::prelude::User::find_by_id(user_id).one(db).await {
            crate::notify::in_app(
                db,
                tenant_id,
                &u,
                "missed_punch",
                &serde_json::json!({ "when": rules.local_date(entry.started_at).to_string() }),
                Some(("time_entry", entry.id)),
                "missed_punch",
            )
            .await;
        }
        closed += 1;
    }
    Ok(closed)
}

/// The per-tenant back-office scan: closes missed punches every run, and on
/// Mondays tells the office about unapproved time from before this week.
pub const SCAN_KIND: &str = "workforce_scan";

/// How often the scan runs.
const SCAN_EVERY_SECS: i64 = 15 * 60;

/// Ensure every tenant has one live `workforce_scan` job (boot + provisioning).
pub async fn ensure_recurring_jobs(db: &sea_orm::DatabaseConnection) {
    let tenants = match entity::prelude::Tenant::find().all(db).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("workforce: tenant scan failed: {e}");
            return;
        }
    };
    for t in tenants {
        let live = entity::prelude::BackgroundJob::find()
            .filter(entity::background_job::Column::TenantId.eq(t.id))
            .filter(entity::background_job::Column::Kind.eq(SCAN_KIND))
            .filter(entity::background_job::Column::Status.is_in([
                "pending",
                "running",
                "awaiting_callback",
            ]))
            .one(db)
            .await;
        if matches!(live, Ok(None)) {
            if let Err(e) =
                crate::scheduler::enqueue(db, t.id, SCAN_KIND, serde_json::json!({}), 30).await
            {
                tracing::error!("workforce: scheduling the scan for {} failed: {e}", t.id);
            }
        }
    }
}

/// Run one scan, then go back to sleep.
pub async fn handle_scan_job(
    db: &sea_orm::DatabaseConnection,
    job: &entity::background_job::Model,
) -> crate::modules::JobOutcome {
    let tenant_id = job.tenant_id;
    let now = Utc::now();
    let mut summary = serde_json::json!({});
    match sweep_missed_punches(db, tenant_id, now).await {
        Ok(n) => summary["missed_punches_closed"] = serde_json::json!(n),
        Err(e) => tracing::error!("workforce: missed-punch sweep failed: {e:?}"),
    }
    match remind_unapproved(db, tenant_id, now).await {
        Ok(n) => summary["unapproved_reminded"] = serde_json::json!(n),
        Err(e) => tracing::error!("workforce: unapproved reminder failed: {e}"),
    }
    let mut out = crate::modules::JobOutcome::reschedule("pending", SCAN_EVERY_SECS);
    out.result = Some(summary);
    out
}

/// Monday from 7 AM local: one note to `team:manage` holders listing unapproved
/// time from before this week (deduplicated per week by the inbox key).
async fn remind_unapproved(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    now: DateTime<Utc>,
) -> Result<usize, DbErr> {
    use chrono::{Datelike, Timelike, Weekday};
    let rules = Rules::load(db, tenant_id).await;
    let local = now.with_timezone(&rules.tz);
    if local.weekday() != Weekday::Mon || local.hour() < 7 {
        return Ok(0);
    }
    let monday = local.date_naive();
    let waiting = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::EndedAt.is_not_null())
        .filter(entity::time_entry::Column::ApprovedAt.is_null())
        .filter(entity::time_entry::Column::StartedAt.lt(rules.day_start(monday)))
        .all(db)
        .await?;
    if waiting.is_empty() {
        return Ok(0);
    }
    let missed = waiting.iter().filter(|e| needs_review(e)).count();
    let mut people: BTreeMap<Uuid, usize> = BTreeMap::new();
    for e in &waiting {
        *people.entry(e.user_id).or_default() += 1;
    }
    let names: HashMap<Uuid, String> = entity::prelude::User::find()
        .filter(entity::user::Column::Id.is_in(people.keys().copied().collect::<Vec<_>>()))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    let list = people
        .iter()
        .map(|(u, n)| format!("{} ({n})", names.get(u).cloned().unwrap_or_default()))
        .collect::<Vec<_>>()
        .join(", ");
    // A stable id per week keeps the reminder to once a week.
    let week_key = Uuid::from_u128(monday.num_days_from_ce() as u128);
    crate::notify::notify_staff(
        db,
        tenant_id,
        "team:manage",
        "time_unapproved",
        serde_json::json!({ "count": waiting.len(), "missed": missed, "people": list }),
        Some(("team_week", week_key)),
        "unapproved",
        None,
    )
    .await;
    Ok(waiting.len())
}

/// One person-week, split by the overtime rule.
#[derive(Debug, Clone)]
pub struct PersonWeek {
    pub user_id: Uuid,
    pub monday: NaiveDate,
    pub split: Split,
    pub days_worked: usize,
    pub entries: usize,
    /// The rate the week is paid at (the latest frozen rate in it).
    pub rate_cents: i64,
    pub gross_cents: i64,
    pub premium_cents: i64,
}

/// Every whole Monday–Sunday week touching `[from, to]` for the given closed
/// entries: each person-week split, and each entry's share of its week's
/// overtime premium (spread across the week by minutes, as Alpha does).
pub fn week_splits(
    entries: &[entity::time_entry::Model],
    profiles: &HashMap<Uuid, entity::employee_profile::Model>,
    rules: &Rules,
) -> (Vec<PersonWeek>, HashMap<Uuid, i64>) {
    let now = Utc::now();
    let mut groups: BTreeMap<(Uuid, NaiveDate), Vec<&entity::time_entry::Model>> = BTreeMap::new();
    for e in entries.iter().filter(|e| e.ended_at.is_some()) {
        let day = rules.local_date(e.started_at);
        groups
            .entry((e.user_id, overtime::week_start(day)))
            .or_default()
            .push(e);
    }
    let mut weeks = Vec::new();
    let mut share = HashMap::new();
    for ((user_id, monday), mut list) in groups {
        list.sort_by_key(|e| e.started_at);
        let mut by_day: BTreeMap<NaiveDate, i64> = BTreeMap::new();
        for e in &list {
            *by_day.entry(rules.local_date(e.started_at)).or_default() += entry_minutes(e, now);
        }
        let eligible = profiles
            .get(&user_id)
            .map(|p| p.employment_type != "contractor")
            .unwrap_or(true);
        let split = overtime::split_week(&by_day, rules.overtime, eligible);
        let rate = list
            .last()
            .and_then(|e| e.pay_rate_cents)
            .or_else(|| profiles.get(&user_id).map(|p| p.pay_rate_cents))
            .unwrap_or(0);
        let premium = split.premium_cents(rate);
        let total = split.minutes();
        for e in &list {
            let m = entry_minutes(e, now);
            let s = if total > 0 {
                overtime::div_round(premium * m, total)
            } else {
                0
            };
            share.insert(e.id, s);
        }
        weeks.push(PersonWeek {
            user_id,
            monday,
            split,
            days_worked: by_day.values().filter(|m| **m > 0).count(),
            entries: list.len(),
            rate_cents: rate,
            gross_cents: split.pay_cents(rate),
            premium_cents: premium,
        });
    }
    (weeks, share)
}

/// All closed entries of the whole weeks that touch `[from, to]` — overtime is
/// decided per whole week, so a report never splits one.
pub async fn entries_for_weeks(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    user_id: Option<Uuid>,
    rules: &Rules,
) -> Result<Vec<entity::time_entry::Model>, DbErr> {
    let start = rules.day_start(overtime::week_start(from));
    let end = rules.day_start(overtime::week_start(to) + Duration::days(7));
    let mut q = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::EndedAt.is_not_null())
        .filter(entity::time_entry::Column::StartedAt.gte(start))
        .filter(entity::time_entry::Column::StartedAt.lt(end));
    if let Some(u) = user_id {
        q = q.filter(entity::time_entry::Column::UserId.eq(u));
    }
    q.order_by_asc(entity::time_entry::Column::StartedAt)
        .all(db)
        .await
}

/// Every profile in the workspace, by user.
pub async fn profiles_by_user(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<HashMap<Uuid, entity::employee_profile::Model>, DbErr> {
    Ok(EmployeeProfile::find()
        .filter(entity::employee_profile::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .into_iter()
        .map(|p| (p.user_id, p))
        .collect())
}

/// Parse a `YYYY-MM-DD` date parameter.
pub fn parse_date(s: &str, what: &str) -> ApiResult<NaiveDate> {
    s.trim()
        .parse::<NaiveDate>()
        .map_err(|_| ApiError::BadRequest(format!("{what} must be a date like 2026-09-30")))
}

/// Parse an RFC 3339 timestamp parameter.
pub fn parse_instant(s: &str, what: &str) -> ApiResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s.trim())
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| {
            ApiError::BadRequest(format!(
                "{what} must be a time like 2026-09-30T08:00:00-07:00"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_is_sane() {
        // Hesperia → Victorville city halls, roughly 11 km.
        let a = Fix {
            lat: 34.4264,
            lng: -117.3009,
        };
        let b = Fix {
            lat: 34.5362,
            lng: -117.2928,
        };
        let d = haversine_m(a, b);
        assert!((11_000.0..13_000.0).contains(&d), "{d}");
        assert!(haversine_m(a, a) < 0.001);
    }

    #[test]
    fn minutes_cost_rounds_once() {
        assert_eq!(minutes_cost(90, 2500), 3750);
        assert_eq!(minutes_cost(7, 1999), 233);
    }
}
