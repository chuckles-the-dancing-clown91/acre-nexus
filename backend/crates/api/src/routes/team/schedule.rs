//! Shifts (the plan) and time off (vacation, sick, personal, unpaid).

use super::time::window;
use super::{my_profile, parse_id};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::tenancy::TenantScope;
use crate::workforce::{self, parse_date, parse_instant, Rules};
use chrono::{Duration, Utc};
use entity::prelude::{TimeOffRequest, User, WorkShift};
use rocket::serde::json::Json;
use rocket::{delete, get, patch, post};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

const SHIFT_KINDS: &[&str] = &["work", "on_call", "training"];
const TIME_OFF_KINDS: &[&str] = &["vacation", "sick", "personal", "unpaid"];

#[derive(Serialize, schemars::JsonSchema)]
pub struct ShiftDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub starts_at: String,
    pub ends_at: String,
    /// `work` | `on_call` | `training`
    pub kind: String,
    pub property_id: Option<Uuid>,
    pub notes: Option<String>,
    pub minutes: i64,
}

async fn names(db: &impl ConnectionTrait, ids: Vec<Uuid>) -> ApiResult<HashMap<Uuid, String>> {
    Ok(User::find()
        .filter(entity::user::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|u| (u.id, u.name))
        .collect())
}

async fn shift_dtos(
    db: &impl ConnectionTrait,
    shifts: Vec<entity::work_shift::Model>,
) -> ApiResult<Vec<ShiftDto>> {
    let n = names(db, shifts.iter().map(|s| s.user_id).collect()).await?;
    Ok(shifts
        .into_iter()
        .map(|s| ShiftDto {
            id: s.id,
            user_id: s.user_id,
            user_name: n.get(&s.user_id).cloned().unwrap_or_default(),
            minutes: (s.ends_at - s.starts_at).num_minutes(),
            starts_at: s.starts_at.to_rfc3339(),
            ends_at: s.ends_at.to_rfc3339(),
            kind: s.kind,
            property_id: s.property_id,
            notes: s.notes,
        })
        .collect())
}

async fn shifts_in(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    from: Option<&str>,
    to: Option<&str>,
    user_id: Option<Uuid>,
) -> ApiResult<Vec<entity::work_shift::Model>> {
    let rules = Rules::load(db, tenant_id).await;
    let (_, _, start, end) = window(from, to, &rules)?;
    let mut q = WorkShift::find()
        .filter(entity::work_shift::Column::TenantId.eq(tenant_id))
        .filter(entity::work_shift::Column::StartsAt.lt(end))
        .filter(entity::work_shift::Column::EndsAt.gt(start));
    if let Some(u) = user_id {
        q = q.filter(entity::work_shift::Column::UserId.eq(u));
    }
    Ok(q.order_by_asc(entity::work_shift::Column::StartsAt)
        .all(db)
        .await?)
}

/// `GET /team/shifts?from&to&user_id` — the schedule.
#[rocket_okapi::openapi(tag = "Team")]
#[get("/team/shifts?<from>&<to>&<user_id>")]
pub async fn list_shifts(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
    user_id: Option<String>,
) -> ApiResult<Json<Vec<ShiftDto>>> {
    user.require(Permission::TeamRead)?;
    let uid = match user_id.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => Some(parse_id(s, "user")?),
        None => None,
    };
    let shifts = shifts_in(&db, scope.tenant_id, from.as_deref(), to.as_deref(), uid).await?;
    Ok(Json(shift_dtos(&db, shifts).await?))
}

/// `GET /me/shifts?from&to` — my schedule.
#[rocket_okapi::openapi(tag = "Me")]
#[get("/me/shifts?<from>&<to>")]
pub async fn my_shifts(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<String>,
    to: Option<String>,
) -> ApiResult<Json<Vec<ShiftDto>>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    let shifts = shifts_in(
        &db,
        scope.tenant_id,
        from.as_deref(),
        to.as_deref(),
        Some(user.user_id),
    )
    .await?;
    Ok(Json(shift_dtos(&db, shifts).await?))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ShiftReq {
    pub user_id: Uuid,
    pub starts_at: String,
    pub ends_at: String,
    pub kind: Option<String>,
    pub property_id: Option<Uuid>,
    pub notes: Option<String>,
}

async fn check_shift(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    b: &ShiftReq,
) -> ApiResult<(chrono::DateTime<Utc>, chrono::DateTime<Utc>, String)> {
    workforce::profile(db, tenant_id, b.user_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("that person isn't on the team".into()))?;
    let start = parse_instant(&b.starts_at, "starts_at")?;
    let end = parse_instant(&b.ends_at, "ends_at")?;
    if end <= start {
        return Err(ApiError::BadRequest(
            "a shift must end after it starts".into(),
        ));
    }
    if end - start > Duration::hours(24) {
        return Err(ApiError::BadRequest(
            "a shift can't be longer than a day".into(),
        ));
    }
    let kind = b.kind.clone().unwrap_or_else(|| "work".into());
    if !SHIFT_KINDS.contains(&kind.as_str()) {
        return Err(ApiError::BadRequest(
            "kind must be work, on_call or training".into(),
        ));
    }
    Ok((start, end, kind))
}

/// `POST /team/shifts`.
#[rocket_okapi::openapi(tag = "Team")]
#[post("/team/shifts", data = "<body>")]
pub async fn create_shift(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ShiftReq>,
) -> ApiResult<Json<ShiftDto>> {
    user.require(Permission::TeamManage)?;
    let (start, end, kind) = check_shift(&db, scope.tenant_id, &body).await?;
    let s = entity::work_shift::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        user_id: Set(body.user_id),
        starts_at: Set(start.into()),
        ends_at: Set(end.into()),
        kind: Set(kind),
        property_id: Set(body.property_id),
        notes: Set(body.notes.clone().filter(|n| !n.trim().is_empty())),
        created_by: Set(Some(user.user_id)),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::SHIFT_CREATE,
        Some("work_shift"),
        Some(s.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(shift_dtos(&db, vec![s]).await?.remove(0)))
}

async fn find_shift(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::work_shift::Model> {
    WorkShift::find_by_id(parse_id(id, "shift")?)
        .filter(entity::work_shift::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("shift not found".into()))
}

/// `PATCH /team/shifts/<id>`.
#[rocket_okapi::openapi(tag = "Team")]
#[patch("/team/shifts/<id>", data = "<body>")]
pub async fn update_shift(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ShiftReq>,
) -> ApiResult<Json<ShiftDto>> {
    user.require(Permission::TeamManage)?;
    let s = find_shift(&db, scope.tenant_id, id).await?;
    let (start, end, kind) = check_shift(&db, scope.tenant_id, &body).await?;
    let mut am: entity::work_shift::ActiveModel = s.into();
    am.user_id = Set(body.user_id);
    am.starts_at = Set(start.into());
    am.ends_at = Set(end.into());
    am.kind = Set(kind);
    am.property_id = Set(body.property_id);
    am.notes = Set(body.notes.clone().filter(|n| !n.trim().is_empty()));
    let s = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::SHIFT_UPDATE,
        Some("work_shift"),
        Some(s.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(shift_dtos(&db, vec![s]).await?.remove(0)))
}

/// `DELETE /team/shifts/<id>`.
#[rocket_okapi::openapi(tag = "Team")]
#[delete("/team/shifts/<id>")]
pub async fn delete_shift(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::TeamManage)?;
    let s = find_shift(&db, scope.tenant_id, id).await?;
    WorkShift::delete_by_id(s.id).exec(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::SHIFT_DELETE,
        Some("work_shift"),
        Some(s.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ---- Time off ---------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct TimeOffDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub starts_on: String,
    pub ends_on: String,
    pub days: i64,
    /// `vacation` | `sick` | `personal` | `unpaid`
    pub kind: String,
    /// `pending` | `approved` | `denied` | `cancelled`
    pub status: String,
    pub reason: Option<String>,
    pub reviewed_by: Option<String>,
    pub review_note: Option<String>,
    pub created_at: String,
}

async fn time_off_dtos(
    db: &impl ConnectionTrait,
    rows: Vec<entity::time_off_request::Model>,
) -> ApiResult<Vec<TimeOffDto>> {
    let mut ids: Vec<Uuid> = rows.iter().map(|r| r.user_id).collect();
    ids.extend(rows.iter().filter_map(|r| r.reviewed_by));
    let n = names(db, ids).await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let days = match (
                r.starts_on.parse::<chrono::NaiveDate>(),
                r.ends_on.parse::<chrono::NaiveDate>(),
            ) {
                (Ok(a), Ok(b)) => (b - a).num_days() + 1,
                _ => 0,
            };
            TimeOffDto {
                id: r.id,
                user_id: r.user_id,
                user_name: n.get(&r.user_id).cloned().unwrap_or_default(),
                starts_on: r.starts_on,
                ends_on: r.ends_on,
                days,
                kind: r.kind,
                status: r.status,
                reason: r.reason,
                reviewed_by: r.reviewed_by.and_then(|u| n.get(&u).cloned()),
                review_note: r.review_note,
                created_at: r.created_at.to_rfc3339(),
            }
        })
        .collect())
}

/// `GET /team/time-off?status` — requests (pending first).
#[rocket_okapi::openapi(tag = "Team")]
#[get("/team/time-off?<status>")]
pub async fn list_time_off(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    status: Option<String>,
) -> ApiResult<Json<Vec<TimeOffDto>>> {
    user.require(Permission::TeamRead)?;
    let mut q = TimeOffRequest::find()
        .filter(entity::time_off_request::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.is_empty()) {
        q = q.filter(entity::time_off_request::Column::Status.eq(s));
    }
    let mut rows = q
        .order_by_desc(entity::time_off_request::Column::StartsOn)
        .all(&db)
        .await?;
    rows.sort_by_key(|r| r.status != "pending");
    Ok(Json(time_off_dtos(&db, rows).await?))
}

/// `GET /me/time-off` — my requests.
#[rocket_okapi::openapi(tag = "Me")]
#[get("/me/time-off")]
pub async fn my_time_off(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<TimeOffDto>>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    let rows = TimeOffRequest::find()
        .filter(entity::time_off_request::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::time_off_request::Column::UserId.eq(user.user_id))
        .order_by_desc(entity::time_off_request::Column::StartsOn)
        .all(&db)
        .await?;
    Ok(Json(time_off_dtos(&db, rows).await?))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TimeOffReq {
    pub starts_on: String,
    /// Inclusive.
    pub ends_on: String,
    pub kind: Option<String>,
    pub reason: Option<String>,
}

/// `POST /me/time-off` — ask for time off; the office is told.
#[rocket_okapi::openapi(tag = "Me")]
#[post("/me/time-off", data = "<body>")]
pub async fn request_time_off(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<TimeOffReq>,
) -> ApiResult<Json<TimeOffDto>> {
    my_profile(&db, scope.tenant_id, &user).await?;
    let a = parse_date(&body.starts_on, "starts_on")?;
    let b = parse_date(&body.ends_on, "ends_on")?;
    if b < a {
        return Err(ApiError::BadRequest(
            "time off can't end before it starts".into(),
        ));
    }
    if (b - a).num_days() > 90 {
        return Err(ApiError::BadRequest(
            "ask for at most 90 days at a time".into(),
        ));
    }
    let kind = body.kind.clone().unwrap_or_else(|| "vacation".into());
    if !TIME_OFF_KINDS.contains(&kind.as_str()) {
        return Err(ApiError::BadRequest(
            "kind must be vacation, sick, personal or unpaid".into(),
        ));
    }
    let row = entity::time_off_request::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        user_id: Set(user.user_id),
        starts_on: Set(a.to_string()),
        ends_on: Set(b.to_string()),
        kind: Set(kind.clone()),
        status: Set("pending".into()),
        reason: Set(body.reason.clone().filter(|r| !r.trim().is_empty())),
        reviewed_by: Set(None),
        reviewed_at: Set(None),
        review_note: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TIME_OFF_REQUEST,
        Some("time_off_request"),
        Some(row.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "kind": kind })),
    )
    .await;
    let name = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .map(|u| u.name)
        .unwrap_or_default();
    crate::notify::notify_staff(
        &db,
        scope.tenant_id,
        "team:manage",
        "time_off_submitted",
        serde_json::json!({ "employee": name, "kind": kind, "starts_on": row.starts_on, "ends_on": row.ends_on }),
        Some(("time_off_request", row.id)),
        "submitted",
        Some(user.user_id),
    )
    .await;
    Ok(Json(time_off_dtos(&db, vec![row]).await?.remove(0)))
}

/// `DELETE /me/time-off/<id>` — cancel my request (pending, or approved and not
/// started yet).
#[rocket_okapi::openapi(tag = "Me")]
#[delete("/me/time-off/<id>")]
pub async fn cancel_time_off(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TimeOffDto>> {
    let r = TimeOffRequest::find_by_id(parse_id(id, "time off")?)
        .filter(entity::time_off_request::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::time_off_request::Column::UserId.eq(user.user_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("request not found".into()))?;
    let rules = Rules::load(&db, scope.tenant_id).await;
    let today = rules.local_date(Utc::now().into()).to_string();
    let cancellable = r.status == "pending" || (r.status == "approved" && r.starts_on > today);
    if !cancellable {
        return Err(ApiError::Conflict(
            "this request can't be cancelled now".into(),
        ));
    }
    let mut am: entity::time_off_request::ActiveModel = r.into();
    am.status = Set("cancelled".into());
    let r = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TIME_OFF_CANCEL,
        Some("time_off_request"),
        Some(r.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(time_off_dtos(&db, vec![r]).await?.remove(0)))
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ReviewReq {
    pub approve: bool,
    pub note: Option<String>,
}

/// `POST /team/time-off/<id>/review` — approve or deny a pending request.
#[rocket_okapi::openapi(tag = "Team")]
#[post("/team/time-off/<id>/review", data = "<body>")]
pub async fn review_time_off(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ReviewReq>,
) -> ApiResult<Json<TimeOffDto>> {
    user.require(Permission::TeamManage)?;
    let r = TimeOffRequest::find_by_id(parse_id(id, "time off")?)
        .filter(entity::time_off_request::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("request not found".into()))?;
    if r.status != "pending" {
        return Err(ApiError::Conflict(
            "only a pending request can be reviewed".into(),
        ));
    }
    let decision = if body.approve { "approved" } else { "denied" };
    let note = body.note.clone().filter(|n| !n.trim().is_empty());
    let mut am: entity::time_off_request::ActiveModel = r.into();
    am.status = Set(decision.into());
    am.reviewed_by = Set(Some(user.user_id));
    am.reviewed_at = Set(Some(Utc::now().into()));
    am.review_note = Set(note.clone());
    let r = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TIME_OFF_REVIEW,
        Some("time_off_request"),
        Some(r.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "decision": decision })),
    )
    .await;
    if let Some(u) = User::find_by_id(r.user_id).one(&db).await? {
        crate::notify::in_app(
            &db,
            scope.tenant_id,
            &u,
            "time_off_reviewed",
            &serde_json::json!({
                "kind": r.kind, "starts_on": r.starts_on, "ends_on": r.ends_on,
                "decision": decision,
                "note": note.map(|n| format!(" Note: {n}")).unwrap_or_default(),
            }),
            Some(("time_off_request", r.id)),
            decision,
        )
        .await;
    }
    Ok(Json(time_off_dtos(&db, vec![r]).await?.remove(0)))
}
