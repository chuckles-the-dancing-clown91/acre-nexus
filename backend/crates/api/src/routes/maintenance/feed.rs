//! The work order's feed and its time: everything that happened on it, in one
//! stream (optionally narrowed to one task), and the hours the crew logged.

use super::desk::ticket_in_reach;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::{Access, TenantScope};
use crate::ticket_feed::{self, FeedItem};
use crate::workforce::entry_minutes;
use chrono::Utc;
use entity::prelude::{TimeEntry, User};
use rocket::serde::json::Json;
use rocket::{get, post};
use schemars::JsonSchema;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// `GET /tickets/<id>/feed?task_id` — everything that happened, newest first.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/tickets/<id>/feed?<task_id>")]
pub async fn feed(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    task_id: Option<&str>,
) -> ApiResult<Json<Vec<FeedItem>>> {
    user.require(Permission::MaintenanceRead)?;
    let t = ticket_in_reach(&db, scope.tenant_id, &access, id).await?;
    let task = match task_id.filter(|s| !s.is_empty()) {
        Some(s) => {
            Some(Uuid::parse_str(s).map_err(|_| ApiError::NotFound("task not found".into()))?)
        }
        None => None,
    };
    let store = ObjectStore::from_env().ok();
    let sign = |key: &str| {
        store
            .as_ref()
            .and_then(|s| s.signed_get_url(key, SIGNED_URL_TTL_SECS).ok())
            .map(|s| s.url)
    };
    Ok(Json(
        ticket_feed::feed(&db, scope.tenant_id, t.id, task, &sign).await?,
    ))
}

#[derive(Serialize, JsonSchema)]
pub struct TimeRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub minutes: i64,
    pub running: bool,
    pub notes: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct TicketTime {
    /// Whether the crew can log time on this work order.
    pub tracking: bool,
    pub total_minutes: i64,
    pub entries: Vec<TimeRow>,
    /// The caller's own running clock on this work order.
    pub my_running: Option<Uuid>,
}

async fn time_rows(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    ticket: &entity::maintenance_ticket::Model,
    me: Uuid,
) -> ApiResult<TicketTime> {
    let rows = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::MaintenanceTicketId.eq(ticket.id))
        .order_by_desc(entity::time_entry::Column::StartedAt)
        .all(db)
        .await?;
    let names: HashMap<Uuid, String> = if rows.is_empty() {
        HashMap::new()
    } else {
        User::find()
            .filter(
                entity::user::Column::Id.is_in(rows.iter().map(|e| e.user_id).collect::<Vec<_>>()),
            )
            .all(db)
            .await?
            .into_iter()
            .map(|u| {
                (
                    u.id,
                    if u.name.trim().is_empty() {
                        u.email
                    } else {
                        u.name
                    },
                )
            })
            .collect()
    };
    let now = Utc::now();
    let mut total = 0;
    let mut my_running = None;
    let entries = rows
        .into_iter()
        .map(|e| {
            let minutes = entry_minutes(&e, now);
            total += minutes;
            if e.ended_at.is_none() && e.user_id == me {
                my_running = Some(e.id);
            }
            TimeRow {
                id: e.id,
                user_name: names.get(&e.user_id).cloned().unwrap_or_default(),
                user_id: e.user_id,
                started_at: e.started_at.to_rfc3339(),
                ended_at: e.ended_at.map(|d| d.to_rfc3339()),
                minutes,
                running: e.ended_at.is_none(),
                notes: e.notes,
            }
        })
        .collect();
    Ok(TicketTime {
        tracking: ticket.track_time,
        total_minutes: total,
        entries,
        my_running,
    })
}

/// `GET /tickets/<id>/time` — hours logged on this work order.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[get("/tickets/<id>/time")]
pub async fn time(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
) -> ApiResult<Json<TicketTime>> {
    user.require(Permission::MaintenanceRead)?;
    let t = ticket_in_reach(&db, scope.tenant_id, &access, id).await?;
    Ok(Json(
        time_rows(&db, scope.tenant_id, &t, user.user_id).await?,
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct LogTimeReq {
    /// How long, in minutes (ends now, or at `ended_at`).
    pub minutes: Option<i64>,
    /// RFC 3339; with `ended_at` instead of `minutes`.
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub notes: Option<String>,
    /// Log it for someone else (needs permission to manage the team).
    pub user_id: Option<Uuid>,
}

/// `POST /tickets/<id>/time` — log time worked on this work order.
#[rocket_okapi::openapi(tag = "Service Desk")]
#[post("/tickets/<id>/time", data = "<body>")]
pub async fn log_time(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: Access,
    id: &str,
    body: Json<LogTimeReq>,
) -> ApiResult<Json<TicketTime>> {
    user.require(Permission::MaintenanceManage)?;
    let t = ticket_in_reach(&db, scope.tenant_id, &access, id).await?;
    if !t.track_time {
        return Err(ApiError::BadRequest(
            "time tracking is off for this work order".into(),
        ));
    }
    let b = body.into_inner();
    let who = match b.user_id {
        Some(u) if u != user.user_id => {
            user.require(Permission::TeamManage)?;
            u
        }
        _ => user.user_id,
    };
    let (start, end) = match (b.minutes, b.started_at.as_deref(), b.ended_at.as_deref()) {
        (Some(m), _, e) if (1..=1440).contains(&m) => {
            let end = match e.filter(|s| !s.trim().is_empty()) {
                Some(s) => crate::workforce::parse_instant(s, "ended_at")?,
                None => Utc::now(),
            };
            (end - chrono::Duration::minutes(m), end)
        }
        (Some(_), _, _) => {
            return Err(ApiError::BadRequest("minutes must be 1 to 1440".into()));
        }
        (None, Some(s), Some(e)) => (
            crate::workforce::parse_instant(s, "started_at")?,
            crate::workforce::parse_instant(e, "ended_at")?,
        ),
        _ => {
            return Err(ApiError::BadRequest(
                "say how many minutes, or when it started and ended".into(),
            ))
        }
    };
    let req = crate::routes::team::EntryReq {
        target: crate::routes::team::TargetReq {
            kind: "work_order".into(),
            maintenance_ticket_id: Some(t.id),
            rehab_project_id: None,
            property_id: None,
        },
        started_at: start.to_rfc3339(),
        ended_at: Some(end.to_rfc3339()),
        break_minutes: 0,
        notes: b.notes,
        user_id: None,
    };
    let saved =
        crate::routes::team::time::save_entry(&db, scope.tenant_id, who, &req, None, false).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TIME_ENTRY_CREATE,
        Some("maintenance_ticket"),
        Some(t.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "entry_id": saved.id, "minutes": (end - start).num_minutes(), "for": who })),
    )
    .await;
    Ok(Json(
        time_rows(&db, scope.tenant_id, &t, user.user_id).await?,
    ))
}
