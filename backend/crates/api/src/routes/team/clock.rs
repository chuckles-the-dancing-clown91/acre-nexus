//! Self-service: the clock, your own time, your hours, and the work you can
//! clock onto. Needs only an employee profile in the workspace.

use super::time::{audit, find_entry, save_entry, window};
use super::{entry_dtos, is_billed, my_profile, EntryReq, Location, TargetReq, TimeEntryDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::tenancy::TenantScope;
use crate::workforce::{
    self, entry_minutes, needs_review, overtime, parse_instant, resolve_target, Rules,
};
use chrono::{Duration, Utc};
use entity::prelude::{MaintenanceTicket, Property, RehabProject, TimeEntry};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct ClockDto {
    /// The running entry, when clocked in.
    pub open: Option<TimeEntryDto>,
    pub today_minutes: i64,
    pub week_minutes: i64,
    /// Missed punches waiting for you to say when you finished.
    pub missed_punches: usize,
    /// Whether the phone's location is noted at clock-in / clock-out.
    pub records_location: bool,
    pub overtime_rule: String,
}

async fn clock_state(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user: &AuthUser,
) -> ApiResult<ClockDto> {
    let rules = Rules::load(db, tenant_id).await;
    let now = Utc::now();
    let today = rules.local_date(now.into());
    let monday = overtime::week_start(today);
    let week = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::UserId.eq(user.user_id))
        .filter(entity::time_entry::Column::StartedAt.gte(rules.day_start(monday)))
        .all(db)
        .await?;
    let today_minutes = week
        .iter()
        .filter(|e| rules.local_date(e.started_at) == today)
        .map(|e| entry_minutes(e, now))
        .sum();
    let week_minutes = week.iter().map(|e| entry_minutes(e, now)).sum();
    let missed = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::UserId.eq(user.user_id))
        .filter(entity::time_entry::Column::MissedPunch.eq(true))
        .filter(entity::time_entry::Column::ResolvedAt.is_null())
        .filter(entity::time_entry::Column::ClaimedEnd.is_null())
        .all(db)
        .await?
        .len();
    let open = match workforce::open_entry(db, tenant_id, user.user_id).await? {
        Some(e) => Some(
            entry_dtos(db, tenant_id, vec![e], false, &rules)
                .await?
                .remove(0),
        ),
        None => None,
    };
    Ok(ClockDto {
        open,
        today_minutes,
        week_minutes,
        missed_punches: missed,
        records_location: rules.clock_location,
        overtime_rule: rules.overtime.describe().to_string(),
    })
}

/// `GET /me/clock` — am I clocked in, and my hours today / this week.
#[rocket_okapi::openapi(tag = "Me")]
#[get("/me/clock")]
pub async fn clock(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<ClockDto>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    workforce::sweep_missed_punches(&db, scope.tenant_id, Utc::now()).await?;
    Ok(Json(clock_state(&db, scope.tenant_id, &user).await?))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ClockInReq {
    #[serde(flatten)]
    pub target: TargetReq,
    pub notes: Option<String>,
    pub location: Option<Location>,
}

/// `POST /me/clock/in` — start the clock on a work order, project, property or
/// other time. Anything already running is closed first.
#[rocket_okapi::openapi(tag = "Me")]
#[post("/me/clock/in", data = "<body>")]
pub async fn clock_in(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ClockInReq>,
) -> ApiResult<Json<ClockDto>> {
    let prof = my_profile(&db, scope.tenant_id, &user).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    if !workforce::is_current(&prof, rules.local_date(Utc::now().into())) {
        return Err(ApiError::Forbidden("your employment has ended".into()));
    }
    let target = resolve_target(
        &db,
        scope.tenant_id,
        &body.target.kind,
        body.target.maintenance_ticket_id,
        body.target.rehab_project_id,
        body.target.property_id,
    )
    .await?;
    let e = workforce::clock_in(
        &db,
        scope.tenant_id,
        user.user_id,
        target,
        body.notes.clone().filter(|n| !n.trim().is_empty()),
        Location::fix(body.location.as_ref()),
        &rules,
    )
    .await?;
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_CLOCK_IN,
        &e,
        serde_json::json!({ "kind": e.kind }),
    )
    .await;
    Ok(Json(clock_state(&db, scope.tenant_id, &user).await?))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ClockOutReq {
    pub location: Option<Location>,
    /// Unpaid break taken during this stretch, in minutes.
    pub break_minutes: Option<i32>,
    pub notes: Option<String>,
}

/// `POST /me/clock/out`.
#[rocket_okapi::openapi(tag = "Me")]
#[post("/me/clock/out", data = "<body>")]
pub async fn clock_out(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ClockOutReq>,
) -> ApiResult<Json<ClockDto>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let open = workforce::open_entry(&db, scope.tenant_id, user.user_id)
        .await?
        .ok_or_else(|| ApiError::Conflict("you're not clocked in".into()))?;
    let now = Utc::now();
    if let Some(b) = body.break_minutes {
        if b < 0 || Duration::minutes(b as i64) >= now - open.started_at.with_timezone(&Utc) {
            return Err(ApiError::BadRequest(
                "the break is longer than the time worked".into(),
            ));
        }
    }
    let mut e = workforce::close_entry(
        &db,
        scope.tenant_id,
        open,
        now,
        Location::fix(body.location.as_ref()),
        &rules,
    )
    .await?;
    if body.break_minutes.is_some() || body.notes.is_some() {
        let mut am: entity::time_entry::ActiveModel = e.into();
        if let Some(b) = body.break_minutes {
            am.break_minutes = Set(b);
        }
        if let Some(n) = body.notes.clone().filter(|n| !n.trim().is_empty()) {
            am.notes = Set(Some(n));
        }
        e = am.update(&db).await?;
    }
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_CLOCK_OUT,
        &e,
        serde_json::json!({ "minutes": entry_minutes(&e, now) }),
    )
    .await;
    Ok(Json(clock_state(&db, scope.tenant_id, &user).await?))
}

/// `GET /me/time?from&to` — my entries (default this week).
#[rocket_okapi::openapi(tag = "Me")]
#[get("/me/time?<from>&<to>")]
pub async fn my_time(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
) -> ApiResult<Json<Vec<TimeEntryDto>>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (_, _, start, end) = window(from.as_deref(), to.as_deref(), &rules)?;
    let entries = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::time_entry::Column::UserId.eq(user.user_id))
        .filter(entity::time_entry::Column::StartedAt.gte(start))
        .filter(entity::time_entry::Column::StartedAt.lt(end))
        .order_by_desc(entity::time_entry::Column::StartedAt)
        .all(&db)
        .await?;
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, entries, false, &rules).await?,
    ))
}

/// Your own entry that you may still change: not approved, not billed, and not
/// a missed punch waiting on the office.
async fn my_editable(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user: &AuthUser,
    id: &str,
) -> ApiResult<entity::time_entry::Model> {
    let e = find_entry(db, tenant_id, id).await?;
    if e.user_id != user.user_id {
        return Err(ApiError::NotFound("time entry not found".into()));
    }
    if e.approved_at.is_some() {
        return Err(ApiError::Conflict(
            "this time is approved — ask the office to change it".into(),
        ));
    }
    if needs_review(&e) {
        return Err(ApiError::Conflict(
            "this is a missed punch — tell the office when you finished instead".into(),
        ));
    }
    if is_billed(db, tenant_id, &e).await? {
        return Err(ApiError::Conflict("this time has been billed".into()));
    }
    Ok(e)
}

/// `POST /me/time` — log time you forgot to clock.
#[rocket_okapi::openapi(tag = "Me")]
#[post("/me/time", data = "<body>")]
pub async fn add_time(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<EntryReq>,
) -> ApiResult<Json<TimeEntryDto>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    let e = save_entry(&db, scope.tenant_id, user.user_id, &body, None, false).await?;
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_ENTRY_CREATE,
        &e,
        serde_json::json!({ "by": "self" }),
    )
    .await;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, vec![e], false, &rules)
            .await?
            .remove(0),
    ))
}

/// `PATCH /me/time/<id>` — fix your own unapproved time.
#[rocket_okapi::openapi(tag = "Me")]
#[patch("/me/time/<id>", data = "<body>")]
pub async fn edit_time(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<EntryReq>,
) -> ApiResult<Json<TimeEntryDto>> {
    let e = my_editable(&db, scope.tenant_id, &user, id).await?;
    let open = e.ended_at.is_none();
    let e = save_entry(&db, scope.tenant_id, user.user_id, &body, Some(e), open).await?;
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_ENTRY_UPDATE,
        &e,
        serde_json::json!({ "by": "self" }),
    )
    .await;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, vec![e], false, &rules)
            .await?
            .remove(0),
    ))
}

/// `DELETE /me/time/<id>`.
#[rocket_okapi::openapi(tag = "Me")]
#[delete("/me/time/<id>")]
pub async fn delete_time(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    let e = my_editable(&db, scope.tenant_id, &user, id).await?;
    audit(
        &db,
        user.user_id,
        crate::audit::actions::TIME_ENTRY_DELETE,
        &e,
        serde_json::json!({ "by": "self" }),
    )
    .await;
    TimeEntry::delete_by_id(e.id).exec(&db).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ClaimReq {
    /// When you really finished (RFC 3339).
    pub ended_at: String,
    pub note: Option<String>,
}

/// `POST /me/time/<id>/claim` — on a missed punch, say when you finished.
#[rocket_okapi::openapi(tag = "Me")]
#[post("/me/time/<id>/claim", data = "<body>")]
pub async fn claim(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ClaimReq>,
) -> ApiResult<Json<TimeEntryDto>> {
    let e = find_entry(&db, scope.tenant_id, id).await?;
    if e.user_id != user.user_id {
        return Err(ApiError::NotFound("time entry not found".into()));
    }
    if !needs_review(&e) {
        return Err(ApiError::Conflict("this isn't a missed punch".into()));
    }
    let end = parse_instant(&body.ended_at, "ended_at")?;
    let start = e.started_at.with_timezone(&Utc);
    if end <= start || end - start > Duration::hours(24) {
        return Err(ApiError::BadRequest(
            "the finish must be after the start, within a day".into(),
        ));
    }
    let mut am: entity::time_entry::ActiveModel = e.into();
    am.claimed_end = Set(Some(end.into()));
    am.punch_note = Set(body.note.clone().filter(|n| !n.trim().is_empty()));
    am.updated_at = Set(Utc::now().into());
    let e = am.update(&db).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    Ok(Json(
        entry_dtos(&db, scope.tenant_id, vec![e], false, &rules)
            .await?
            .remove(0),
    ))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct WeekHours {
    pub week_of: String,
    pub days_worked: usize,
    pub minutes: i64,
    pub regular_minutes: i64,
    pub overtime_minutes: i64,
    pub double_minutes: i64,
    pub unapproved_minutes: i64,
    /// Your gross for the week at your rate, before taxes.
    pub gross_cents: i64,
}

/// `GET /me/hours?from&to` — my weeks, split by the overtime rule.
#[rocket_okapi::openapi(tag = "Me")]
#[get("/me/hours?<from>&<to>")]
pub async fn my_hours(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
) -> ApiResult<Json<Vec<WeekHours>>> {
    let prof = my_profile(&db, scope.tenant_id, &user).await?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let (from, to, _, _) = window(from.as_deref(), to.as_deref(), &rules)?;
    let entries =
        workforce::entries_for_weeks(&db, scope.tenant_id, from, to, Some(user.user_id), &rules)
            .await?;
    let profiles = std::collections::HashMap::from([(prof.user_id, prof)]);
    let (weeks, _) = workforce::week_splits(&entries, &profiles, &rules);
    let now = Utc::now();
    let mut out: Vec<WeekHours> = weeks
        .into_iter()
        .map(|w| {
            let unapproved = entries
                .iter()
                .filter(|e| {
                    overtime::week_start(rules.local_date(e.started_at)) == w.monday
                        && e.approved_at.is_none()
                })
                .map(|e| entry_minutes(e, now))
                .sum();
            WeekHours {
                week_of: w.monday.to_string(),
                days_worked: w.days_worked,
                minutes: w.split.minutes(),
                regular_minutes: w.split.regular,
                overtime_minutes: w.split.overtime,
                double_minutes: w.split.double,
                unapproved_minutes: unapproved,
                gross_cents: w.gross_cents,
            }
        })
        .collect();
    out.reverse();
    Ok(Json(out))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct WorkOption {
    /// `work_order` | `project` | `property`
    pub kind: String,
    pub id: Uuid,
    pub label: String,
    pub property_name: Option<String>,
    /// Assigned to me (work orders).
    pub mine: bool,
}

/// `GET /me/work` — what I can clock onto: open work orders (mine first),
/// active rehab projects, and properties.
#[rocket_okapi::openapi(tag = "Me")]
#[get("/me/work")]
pub async fn my_work(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<WorkOption>>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    let props: std::collections::HashMap<Uuid, String> = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    let mut out = Vec::new();
    let tickets = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(
            entity::maintenance_ticket::Column::Status
                .is_in(crate::routes::maintenance::OPEN_STATUSES.to_vec()),
        )
        .order_by_desc(entity::maintenance_ticket::Column::UpdatedAt)
        .limit(300)
        .all(&db)
        .await?;
    for t in tickets {
        out.push(WorkOption {
            kind: "work_order".into(),
            id: t.id,
            label: t.title,
            property_name: props.get(&t.property_id).cloned(),
            mine: t.assignee_user_id == Some(user.user_id),
        });
    }
    for p in RehabProject::find()
        .filter(entity::rehab_project::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::rehab_project::Column::Status.is_in(["planning", "active", "on_hold"]))
        .all(&db)
        .await?
    {
        out.push(WorkOption {
            kind: "project".into(),
            id: p.id,
            label: p.name,
            property_name: props.get(&p.property_id).cloned(),
            mine: false,
        });
    }
    for (id, name) in &props {
        out.push(WorkOption {
            kind: "property".into(),
            id: *id,
            label: name.clone(),
            property_name: None,
            mine: false,
        });
    }
    out.sort_by(|a, b| {
        b.mine
            .cmp(&a.mine)
            .then(kind_rank(&a.kind).cmp(&kind_rank(&b.kind)))
            .then(a.label.cmp(&b.label))
    });
    Ok(Json(out))
}

fn kind_rank(k: &str) -> u8 {
    match k {
        "work_order" => 0,
        "project" => 1,
        _ => 2,
    }
}
