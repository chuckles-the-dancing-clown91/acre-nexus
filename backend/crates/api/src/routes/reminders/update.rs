use super::dto::{ReminderDto, UpdateReminderReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{NaiveDate, Utc};
use entity::prelude::Reminder;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// `PATCH /reminders/<id>` — edit a reminder, mark it done, or cancel it.
/// Re-dating an active reminder re-arms its lead times.
#[rocket_okapi::openapi(tag = "Calendar")]
#[patch("/reminders/<id>", data = "<body>")]
pub async fn update_reminder(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateReminderReq>,
) -> ApiResult<Json<ReminderDto>> {
    user.require(Permission::CalendarManage)?;
    let rid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let reminder = Reminder::find_by_id(rid)
        .filter(entity::reminder::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("reminder not found".into()))?;
    let b = body.into_inner();
    let now = Utc::now();
    let mut am: entity::reminder::ActiveModel = reminder.into();
    if let Some(title) = b.title.filter(|t| !t.trim().is_empty()) {
        am.title = Set(title.trim().to_string());
    }
    if let Some(desc) = b.description {
        am.description = Set(Some(desc).filter(|d| !d.trim().is_empty()));
    }
    if let Some(due) = b.due_date.filter(|d| !d.trim().is_empty()) {
        NaiveDate::parse_from_str(&due, "%Y-%m-%d")
            .map_err(|_| ApiError::BadRequest("due_date must be YYYY-MM-DD".into()))?;
        am.due_date = Set(due);
        am.fired = Set(json!([]));
    }
    if let Some(mut days) = b.lead_days {
        days.retain(|d| (0..=365).contains(d));
        days.sort_unstable();
        days.dedup();
        days.reverse();
        if days.is_empty() {
            return Err(ApiError::BadRequest(
                "lead_days must contain at least one value in 0..=365".into(),
            ));
        }
        am.lead_days = Set(json!(days));
    }
    if let Some(recipients) = b.recipients {
        let cleaned: Vec<String> = recipients
            .into_iter()
            .map(|r| r.trim().to_string())
            .filter(|r| r.contains('@'))
            .collect();
        am.recipients = Set(json!(cleaned));
    }
    if let Some(status) = b.status.filter(|s| !s.is_empty()) {
        if !crate::reminders::STATUSES.contains(&status.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "invalid status '{status}' (expected one of {})",
                crate::reminders::STATUSES.join(", ")
            )));
        }
        am.completed_at = Set((status == "done").then(|| now.into()));
        am.status = Set(status);
    }
    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REMINDER_UPDATE,
        Some("reminder"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "status": saved.status, "due_date": saved.due_date })),
    )
    .await;

    Ok(Json(ReminderDto::from_model(saved, now.date_naive())))
}
