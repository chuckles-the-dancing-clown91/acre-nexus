//! Office timesheets: everyone's time, corrections, approval, missed punches.

use super::{entry_dtos, is_billed, parse_id, sees_pay, EntryReq, TimeEntryDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use crate::workforce::{
    self, entry_minutes, find_overlap, minutes_cost, needs_review, parse_date, parse_instant,
    resolve_target, Rules,
};
use chrono::{DateTime, Duration, Utc};
use entity::prelude::TimeEntry;
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Most entries one list call returns.
const MAX_ENTRIES: usize = 2_000;

/// Shared by the office and self-service: validate and write a manual entry
/// (new when `existing` is `None`).
pub(crate) async fn save_entry(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
    req: &EntryReq,
    existing: Option<entity::time_entry::Model>,
    allow_open: bool,
) -> ApiResult<entity::time_entry::Model> {
    let rules = Rules::load(db, tenant_id).await;
    let target = resolve_target(
        db,
        tenant_id,
        &req.target.kind,
        req.target.maintenance_ticket_id,
        req.target.rehab_project_id,
        req.target.property_id,
    )
    .await?;
    let start = parse_instant(&req.started_at, "started_at")?;
    let end = match req.ended_at.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(s) => Some(parse_instant(s, "ended_at")?),
        None if allow_open => None,
        None => return Err(ApiError::BadRequest("say when the work ended".into())),
    };
    if let Some(end) = end {
        if end <= start {
            return Err(ApiError::BadRequest(
                "an entry must end after it starts".into(),
            ));
        }
        if end - start > Duration::hours(24) {
            return Err(ApiError::BadRequest(
                "one entry can't be longer than 24 hours — split it by day".into(),
            ));
        }
        if req.break_minutes < 0 || Duration::minutes(req.break_minutes as i64) >= end - start {
            return Err(ApiError::BadRequest(
                "the break is longer than the time worked".into(),
            ));
        }
    }
    if start > Utc::now() + Duration::minutes(5) {
        return Err(ApiError::BadRequest("that time hasn't happened yet".into()));
    }
    if end.is_none() {
        if let Some(open) = workforce::open_entry(db, tenant_id, user_id).await? {
            if existing.as_ref().map(|e| e.id) != Some(open.id) {
                return Err(ApiError::Conflict(
                    "they're already clocked in — clock them out first".into(),
                ));
            }
        }
    }
    if let Some(clash) = find_overlap(
        db,
        tenant_id,
        user_id,
        start,
        end,
        existing.as_ref().map(|e| e.id),
    )
    .await?
    {
        let at = clash.started_at.with_timezone(&rules.tz);
        return Err(ApiError::Conflict(format!(
            "this overlaps time already logged from {}{} — edit that entry instead",
            at.format("%b %-d, %-I:%M %p"),
            if clash.ended_at.is_none() {
                " (still clocked in)"
            } else {
                ""
            }
        )));
    }
    let prof = workforce::profile(db, tenant_id, user_id).await?;
    let (pay, bill) = match (&existing, end) {
        // Keep the rates frozen at the first close; a still-open entry has none.
        (Some(e), Some(_)) if e.pay_rate_cents.is_some() => (e.pay_rate_cents, e.bill_rate_cents),
        (_, Some(_)) => (
            Some(prof.as_ref().map(|p| p.pay_rate_cents).unwrap_or(0)),
            Some(prof.as_ref().map(|p| p.bill_rate_cents).unwrap_or(0)),
        ),
        (_, None) => (None, None),
    };
    let now = Utc::now();
    let notes = req.notes.clone().filter(|n| !n.trim().is_empty());
    match existing {
        Some(e) => {
            let mut am: entity::time_entry::ActiveModel = e.into();
            am.kind = Set(target.kind);
            am.maintenance_ticket_id = Set(target.maintenance_ticket_id);
            am.rehab_project_id = Set(target.rehab_project_id);
            am.property_id = Set(target.property_id);
            am.started_at = Set(start.into());
            am.ended_at = Set(end.map(Into::into));
            am.break_minutes = Set(req.break_minutes.max(0));
            am.notes = Set(notes);
            am.pay_rate_cents = Set(pay);
            am.bill_rate_cents = Set(bill);
            // Changed time needs approving again.
            am.approved_at = Set(None);
            am.approved_by = Set(None);
            am.updated_at = Set(now.into());
            Ok(am.update(db).await?)
        }
        None => Ok(entity::time_entry::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            user_id: Set(user_id),
            kind: Set(target.kind),
            maintenance_ticket_id: Set(target.maintenance_ticket_id),
            rehab_project_id: Set(target.rehab_project_id),
            property_id: Set(target.property_id),
            started_at: Set(start.into()),
            ended_at: Set(end.map(Into::into)),
            break_minutes: Set(req.break_minutes.max(0)),
            notes: Set(notes),
            pay_rate_cents: Set(pay),
            bill_rate_cents: Set(bill),
            approved_by: Set(None),
            approved_at: Set(None),
            missed_punch: Set(false),
            missed_punch_reason: Set(None),
            claimed_end: Set(None),
            punch_note: Set(None),
            resolved_by: Set(None),
            resolved_at: Set(None),
            in_lat: Set(None),
            in_lng: Set(None),
            in_distance_m: Set(None),
            out_lat: Set(None),
            out_lng: Set(None),
            out_distance_m: Set(None),
            billed_bill_id: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?),
    }
}

pub(crate) async fn find_entry(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::time_entry::Model> {
    TimeEntry::find_by_id(parse_id(id, "time entry")?)
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("time entry not found".into()))
}

pub(crate) async fn audit(
    db: &impl ConnectionTrait,
    actor: Uuid,
    action: &str,
    entry: &entity::time_entry::Model,
    extra: serde_json::Value,
) {
    crate::audit::record(
        db,
        Some(actor),
        action,
        Some("time_entry"),
        Some(entry.id.to_string()),
        Some(entry.tenant_id),
        Some(serde_json::json!({ "user_id": entry.user_id, "detail": extra })),
    )
    .await;
}

/// Local-day window `[from, to]` → instants, defaulting to the current week.
pub(crate) fn window(
    from: Option<&str>,
    to: Option<&str>,
    rules: &Rules,
) -> ApiResult<(
    chrono::NaiveDate,
    chrono::NaiveDate,
    DateTime<Utc>,
    DateTime<Utc>,
)> {
    let today = rules.local_date(Utc::now().into());
    let monday = workforce::overtime::week_start(today);
    let from = match from {
        Some(s) => parse_date(s, "from")?,
        None => monday,
    };
    let to = match to {
        Some(s) => parse_date(s, "to")?,
        None => monday + Duration::days(6),
    };
    if to < from {
        return Err(ApiError::BadRequest("to is before from".into()));
    }
    if (to - from).num_days() > 400 {
        return Err(ApiError::BadRequest("pick at most about a year".into()));
    }
    Ok((
        from,
        to,
        rules.day_start(from),
        rules.day_start(to + Duration::days(1)),
    ))
}

/// `GET /team/time?from&to&user_id&status` — everyone's time. `status`:
/// `unapproved` | `missed` | `open` | `approved` (default all).
#[rocket_okapi::openapi(tag = "Team")]
#[get("/team/time?<from>&<to>&<user_id>&<status>")]
pub async fn list(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
    user_id: Option<String>,
    status: Option<String>,
) -> ApiResult<Json<Vec<TimeEntryDto>>> {
    user.require(Permission::TeamRead)?;
    workforce::sweep_missed_punches(&db, scope.tenant_id, Utc::now()).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (_, _, start, end) = window(from.as_deref(), to.as_deref(), &rules)?;
    let mut q = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::time_entry::Column::StartedAt.gte(start))
        .filter(entity::time_entry::Column::StartedAt.lt(end));
    if let Some(u) = user_id.as_deref().filter(|s| !s.is_empty()) {
        q = q.filter(entity::time_entry::Column::UserId.eq(parse_id(u, "user")?));
    }
    let mut entries = q
        .order_by_desc(entity::time_entry::Column::StartedAt)
        .all(&db)
        .await?;
    match status.as_deref().unwrap_or("") {
        "unapproved" => entries.retain(|e| e.ended_at.is_some() && e.approved_at.is_none()),
        "missed" => entries.retain(needs_review),
        "open" => entries.retain(|e| e.ended_at.is_none()),
        "approved" => entries.retain(|e| e.approved_at.is_some()),
        _ => {}
    }
    entries.truncate(MAX_ENTRIES);
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, entries, sees_pay(&user), &rules).await?,
    ))
}

/// `POST /team/time` — log time for someone.
#[rocket_okapi::openapi(tag = "Team")]
#[post("/team/time", data = "<body>")]
pub async fn create(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<EntryReq>,
) -> ApiResult<Json<TimeEntryDto>> {
    user.require(Permission::TeamManage)?;
    let who = body
        .user_id
        .ok_or_else(|| ApiError::BadRequest("say whose time this is (user_id)".into()))?;
    workforce::profile(&db, scope.tenant_id, who)
        .await?
        .ok_or_else(|| ApiError::BadRequest("that person isn't on the team".into()))?;
    let e = save_entry(&db, scope.tenant_id, who, &body, None, true).await?;
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_ENTRY_CREATE,
        &e,
        serde_json::json!({ "by": "office" }),
    )
    .await;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, vec![e], sees_pay(&user), &rules)
            .await?
            .remove(0),
    ))
}

/// `PATCH /team/time/<id>` — correct an entry (it needs approving again).
#[rocket_okapi::openapi(tag = "Team")]
#[patch("/team/time/<id>", data = "<body>")]
pub async fn update(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<EntryReq>,
) -> ApiResult<Json<TimeEntryDto>> {
    user.require(Permission::TeamManage)?;
    let e = find_entry(&db, scope.tenant_id, id).await?;
    if is_billed(&db, scope.tenant_id, &e).await? {
        return Err(ApiError::Conflict(
            "this time is on an owner bill — void that bill first".into(),
        ));
    }
    let owner = e.user_id;
    let e = save_entry(&db, scope.tenant_id, owner, &body, Some(e), true).await?;
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_ENTRY_UPDATE,
        &e,
        serde_json::json!({ "by": "office" }),
    )
    .await;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, vec![e], sees_pay(&user), &rules)
            .await?
            .remove(0),
    ))
}

/// `DELETE /team/time/<id>`.
#[rocket_okapi::openapi(tag = "Team")]
#[delete("/team/time/<id>")]
pub async fn remove(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::TeamManage)?;
    let e = find_entry(&db, scope.tenant_id, id).await?;
    if is_billed(&db, scope.tenant_id, &e).await? {
        return Err(ApiError::Conflict(
            "this time is on an owner bill — void that bill first".into(),
        ));
    }
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_ENTRY_DELETE,
        &e,
        serde_json::json!({ "minutes": entry_minutes(&e, Utc::now()) }),
    )
    .await;
    TimeEntry::delete_by_id(e.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn approve_one(
    db: &impl ConnectionTrait,
    actor: Uuid,
    e: entity::time_entry::Model,
) -> ApiResult<entity::time_entry::Model> {
    if e.ended_at.is_none() {
        return Err(ApiError::Conflict("they're still clocked in".into()));
    }
    if needs_review(&e) {
        return Err(ApiError::Conflict(
            "this is a missed punch — confirm or correct the finish time first".into(),
        ));
    }
    if e.approved_at.is_some() {
        return Ok(e);
    }
    let mut am: entity::time_entry::ActiveModel = e.into();
    am.approved_at = Set(Some(Utc::now().into()));
    am.approved_by = Set(Some(actor));
    am.updated_at = Set(Utc::now().into());
    let e = am.update(db).await?;
    audit(
        db,
        actor,
        crate::audit::actions::TIME_APPROVE,
        &e,
        serde_json::json!({}),
    )
    .await;
    Ok(e)
}

/// `POST /team/time/<id>/approve`.
#[rocket_okapi::openapi(tag = "Team")]
#[post("/team/time/<id>/approve")]
pub async fn approve(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TimeEntryDto>> {
    user.require(Permission::TeamManage)?;
    let e = find_entry(&db, scope.tenant_id, id).await?;
    let e = approve_one(&db, user.user_id, e).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, vec![e], sees_pay(&user), &rules)
            .await?
            .remove(0),
    ))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct BulkApproveReq {
    pub ids: Vec<Uuid>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct BulkApproveResp {
    pub approved: usize,
    /// Entries skipped, with why (still open, missed punch to settle…).
    pub skipped: Vec<SkippedEntry>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SkippedEntry {
    pub id: Uuid,
    pub reason: String,
}

/// `POST /team/time/approve` — approve many; what can't be approved is listed.
#[rocket_okapi::openapi(tag = "Team")]
#[post("/team/time/approve", data = "<body>")]
pub async fn approve_many(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<BulkApproveReq>,
) -> ApiResult<Json<BulkApproveResp>> {
    user.require(Permission::TeamManage)?;
    let entries = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::time_entry::Column::Id.is_in(body.ids.clone()))
        .all(&db)
        .await?;
    let mut approved = 0;
    let mut skipped = Vec::new();
    for e in entries {
        let id = e.id;
        match approve_one(&db, user.user_id, e).await {
            Ok(_) => approved += 1,
            Err(ApiError::Conflict(reason)) => skipped.push(SkippedEntry { id, reason }),
            Err(other) => return Err(other),
        }
    }
    Ok(Json(BulkApproveResp { approved, skipped }))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ResolveReq {
    /// The real finish (RFC 3339). Omitted = keep the claimed end, else the guess.
    pub ended_at: Option<String>,
    pub break_minutes: Option<i32>,
}

/// `POST /team/time/<id>/resolve` — settle a missed punch.
#[rocket_okapi::openapi(tag = "Team")]
#[post("/team/time/<id>/resolve", data = "<body>")]
pub async fn resolve(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ResolveReq>,
) -> ApiResult<Json<TimeEntryDto>> {
    user.require(Permission::TeamManage)?;
    let e = find_entry(&db, scope.tenant_id, id).await?;
    if !needs_review(&e) {
        return Err(ApiError::Conflict(
            "this isn't a missed punch waiting on review".into(),
        ));
    }
    let start = e.started_at.with_timezone(&Utc);
    let end = match body.ended_at.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(s) => parse_instant(s, "ended_at")?,
        None => e
            .claimed_end
            .or(e.ended_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or(start + Duration::hours(8)),
    };
    if end <= start || end - start > Duration::hours(24) {
        return Err(ApiError::BadRequest(
            "the finish must be after the start, within a day".into(),
        ));
    }
    if let Some(clash) = find_overlap(
        &db,
        scope.tenant_id,
        e.user_id,
        start,
        Some(end),
        Some(e.id),
    )
    .await?
    {
        return Err(ApiError::Conflict(format!(
            "that finish overlaps another entry starting {}",
            clash.started_at.to_rfc3339()
        )));
    }
    let mut am: entity::time_entry::ActiveModel = e.into();
    am.ended_at = Set(Some(end.into()));
    if let Some(b) = body.break_minutes {
        am.break_minutes = Set(b.max(0));
    }
    am.resolved_at = Set(Some(Utc::now().into()));
    am.resolved_by = Set(Some(user.user_id));
    am.updated_at = Set(Utc::now().into());
    let e = am.update(&db).await?;
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_RESOLVE,
        &e,
        serde_json::json!({ "ended_at": end.to_rfc3339() }),
    )
    .await;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, vec![e], sees_pay(&user), &rules)
            .await?
            .remove(0),
    ))
}

/// `POST /team/time/<id>/clock-out` — clock someone out now (they forgot).
#[rocket_okapi::openapi(tag = "Team")]
#[post("/team/time/<id>/clock-out")]
pub async fn clock_out(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TimeEntryDto>> {
    user.require(Permission::TeamManage)?;
    let e = find_entry(&db, scope.tenant_id, id).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let e = workforce::close_entry(&db, scope.tenant_id, e, Utc::now(), None, &rules).await?;
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_CLOCK_OUT,
        &e,
        serde_json::json!({ "by": "office" }),
    )
    .await;
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, vec![e], sees_pay(&user), &rules)
            .await?
            .remove(0),
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PersonHours {
    pub user_id: Uuid,
    pub name: String,
    pub minutes: i64,
    /// On work orders / rehab projects / properties.
    pub on_work_minutes: i64,
    pub other_minutes: i64,
    pub unapproved_minutes: i64,
    pub missed_punches: usize,
    pub clocked_in: bool,
    /// Only with `payroll:read`.
    pub labor_cost_cents: Option<i64>,
    pub work_orders: usize,
}

/// `GET /team/time/summary?from&to` — hours per person (the crew report).
#[rocket_okapi::openapi(tag = "Team")]
#[get("/team/time/summary?<from>&<to>")]
pub async fn summary(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
) -> ApiResult<Json<Vec<PersonHours>>> {
    user.require(Permission::TeamRead)?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (_, _, start, end) = window(from.as_deref(), to.as_deref(), &rules)?;
    let entries = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::time_entry::Column::StartedAt.gte(start))
        .filter(entity::time_entry::Column::StartedAt.lt(end))
        .all(&db)
        .await?;
    let profiles = workforce::profiles_by_user(&db, scope.tenant_id).await?;
    let users: HashMap<Uuid, String> = entity::prelude::User::find()
        .filter(entity::user::Column::Id.is_in(profiles.keys().copied().collect::<Vec<_>>()))
        .all(&db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect();
    let show_pay = sees_pay(&user);
    let now = Utc::now();
    let mut rows: HashMap<Uuid, PersonHours> = HashMap::new();
    let mut tickets: HashMap<Uuid, std::collections::HashSet<Uuid>> = HashMap::new();
    for p in profiles.values() {
        rows.insert(
            p.user_id,
            PersonHours {
                user_id: p.user_id,
                name: users.get(&p.user_id).cloned().unwrap_or_default(),
                minutes: 0,
                on_work_minutes: 0,
                other_minutes: 0,
                unapproved_minutes: 0,
                missed_punches: 0,
                clocked_in: false,
                labor_cost_cents: show_pay.then_some(0),
                work_orders: 0,
            },
        );
    }
    for e in &entries {
        let Some(r) = rows.get_mut(&e.user_id) else {
            continue;
        };
        let m = entry_minutes(e, now);
        r.minutes += m;
        if matches!(e.kind.as_str(), "work_order" | "project" | "property") {
            r.on_work_minutes += m;
        } else {
            r.other_minutes += m;
        }
        if e.ended_at.is_some() && e.approved_at.is_none() {
            r.unapproved_minutes += m;
        }
        if needs_review(e) {
            r.missed_punches += 1;
        }
        if e.ended_at.is_none() {
            r.clocked_in = true;
        }
        if let Some(c) = r.labor_cost_cents.as_mut() {
            let rate = e
                .pay_rate_cents
                .or(profiles.get(&e.user_id).map(|p| p.pay_rate_cents))
                .unwrap_or(0);
            *c += minutes_cost(m, rate);
        }
        if let Some(t) = e.maintenance_ticket_id {
            tickets.entry(e.user_id).or_default().insert(t);
        }
    }
    let mut out: Vec<PersonHours> = rows
        .into_values()
        .map(|mut r| {
            r.work_orders = tickets.get(&r.user_id).map(|s| s.len()).unwrap_or(0);
            r
        })
        .collect();
    out.sort_by(|a, b| b.minutes.cmp(&a.minutes).then(a.name.cmp(&b.name)));
    Ok(Json(out))
}
