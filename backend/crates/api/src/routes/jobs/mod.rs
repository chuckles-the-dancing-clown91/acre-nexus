//! **Settings → Schedule**: what the background jobs did and when they run
//! next, for this workspace only. Recurring jobs reschedule themselves, so the
//! same row shows when it last ran (`updated_at`, `result`) and when it runs
//! next (`run_at`).

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::BackgroundJob;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::Serialize;
use std::collections::BTreeMap;
use uuid::Uuid;

/// Jobs that run on their own, with what they do, in words.
pub const RECURRING: &[(&str, &str)] = &[
    (
        "billing_cycle",
        "Rent charges, late fees, autopay, bank feeds and the monthly snapshot",
    ),
    (
        "reminder_scan",
        "Calendar reminders for staff, and lease end dates",
    ),
    (
        "resident_reminders",
        "Rent due, rent past due, lease expiry, inspections and warranties",
    ),
    ("manager_digest", "The morning summary email for managers"),
    (
        "helpdesk_scan",
        "Work-order SLA breaches, follow-ups, low stock and preventive plans",
    ),
    (
        "workforce_scan",
        "Missed punches and timesheets waiting for approval",
    ),
    (
        "property_photo_scan",
        "A street photo for any property without one",
    ),
];

#[derive(Serialize, schemars::JsonSchema)]
pub struct JobDto {
    pub id: Uuid,
    pub kind: String,
    /// What the job does, for recurring kinds.
    pub label: Option<String>,
    pub status: String,
    /// When it runs next (or ran, when finished).
    pub run_at: String,
    /// When it last changed: the last run of a recurring job.
    pub updated_at: String,
    pub attempts: i32,
    pub last_error: Option<String>,
    #[schemars(with = "Option<serde_json::Value>")]
    pub result: Option<serde_json::Value>,
}

fn label_of(kind: &str) -> Option<String> {
    RECURRING
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, l)| l.to_string())
}

impl From<entity::background_job::Model> for JobDto {
    fn from(j: entity::background_job::Model) -> Self {
        JobDto {
            label: label_of(&j.kind),
            id: j.id,
            kind: j.kind,
            status: j.status,
            run_at: j.run_at.to_rfc3339(),
            updated_at: j.updated_at.to_rfc3339(),
            attempts: j.attempts,
            last_error: j.last_error,
            result: j.result,
        }
    }
}

/// `GET /admin/jobs/schedule` — one row per recurring job kind: its live job
/// (when it runs next, when it last ran and what it did).
#[rocket_okapi::openapi(tag = "Schedule")]
#[get("/admin/jobs/schedule")]
pub async fn schedule(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<JobDto>>> {
    user.require(Permission::TenantManage)?;
    let kinds: Vec<&str> = RECURRING.iter().map(|(k, _)| *k).collect();
    let rows = BackgroundJob::find()
        .filter(entity::background_job::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::background_job::Column::Kind.is_in(kinds))
        .order_by_desc(entity::background_job::Column::UpdatedAt)
        .all(&db)
        .await?;
    // The newest row per kind is the live one.
    let mut latest: BTreeMap<String, entity::background_job::Model> = BTreeMap::new();
    for r in rows {
        latest.entry(r.kind.clone()).or_insert(r);
    }
    Ok(Json(latest.into_values().map(JobDto::from).collect()))
}

/// `GET /admin/jobs?kind&status&limit&before` — this workspace's jobs, newest
/// change first.
#[rocket_okapi::openapi(tag = "Schedule")]
#[allow(clippy::too_many_arguments)]
#[get("/admin/jobs?<kind>&<status>&<limit>&<before>")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    kind: Option<String>,
    status: Option<String>,
    limit: Option<u64>,
    before: Option<String>,
) -> ApiResult<Json<Vec<JobDto>>> {
    user.require(Permission::TenantManage)?;
    let mut q =
        BackgroundJob::find().filter(entity::background_job::Column::TenantId.eq(scope.tenant_id));
    if let Some(k) = kind.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::background_job::Column::Kind.eq(k));
    }
    if let Some(s) = status.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::background_job::Column::Status.eq(s));
    }
    if let Some(b) = crate::paging::before(before.as_deref())? {
        q = q.filter(entity::background_job::Column::UpdatedAt.lt(b));
    }
    let rows = q
        .order_by_desc(entity::background_job::Column::UpdatedAt)
        .limit(crate::paging::limit(limit, 50, 200))
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(JobDto::from).collect()))
}

/// `POST /admin/jobs/<id>/run-now` — run a waiting job at the next tick.
#[rocket_okapi::openapi(tag = "Schedule")]
#[post("/admin/jobs/<id>/run-now")]
pub async fn run_now(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<JobDto>> {
    user.require(Permission::TenantManage)?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let job = BackgroundJob::find_by_id(id)
        .filter(entity::background_job::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("job not found".into()))?;
    if job.status != "pending" {
        return Err(ApiError::Conflict(format!(
            "only a waiting job can run now (this one is {})",
            job.status
        )));
    }
    let kind = job.kind.clone();
    let mut am: entity::background_job::ActiveModel = job.into();
    am.run_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::JOB_RUN_NOW,
        Some("background_job"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "kind": kind })),
    )
    .await;
    Ok(Json(JobDto::from(saved)))
}
