use super::dto::{CreateReminderReq, ReminderDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::{NaiveDate, Utc};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /reminders` — create a reminder. Lead times default to the
/// workspace's `calendar.default_lead_days` setting.
#[rocket_okapi::openapi(tag = "Calendar")]
#[post("/reminders", data = "<body>")]
pub async fn create_reminder(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateReminderReq>,
) -> ApiResult<Json<ReminderDto>> {
    user.require(Permission::CalendarManage)?;
    let b = body.into_inner();
    if !crate::reminders::SUBJECT_TYPES.contains(&b.subject_type.as_str()) {
        return Err(ApiError::BadRequest(format!(
            "invalid subject_type '{}' (expected one of {})",
            b.subject_type,
            crate::reminders::SUBJECT_TYPES.join(", ")
        )));
    }
    if b.title.trim().is_empty() {
        return Err(ApiError::BadRequest("title is required".into()));
    }
    NaiveDate::parse_from_str(&b.due_date, "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest("due_date must be YYYY-MM-DD".into()))?;

    let lead_days = match b.lead_days {
        Some(mut days) => {
            days.retain(|d| (0..=365).contains(d));
            days.sort_unstable();
            days.dedup();
            days.reverse();
            if days.is_empty() {
                return Err(ApiError::BadRequest(
                    "lead_days must contain at least one value in 0..=365".into(),
                ));
            }
            days
        }
        None => crate::reminders::parse_lead_days(
            &crate::settings::get_string(
                &db,
                scope.tenant_id,
                crate::settings::CALENDAR_DEFAULT_LEAD_DAYS,
            )
            .await,
        ),
    };
    let recipients: Vec<String> = b
        .recipients
        .into_iter()
        .map(|r| r.trim().to_string())
        .filter(|r| r.contains('@'))
        .collect();

    let now = Utc::now();
    let saved = entity::reminder::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        subject_type: Set(b.subject_type),
        subject_id: Set(b.subject_id),
        title: Set(b.title.trim().to_string()),
        description: Set(b.description.filter(|d| !d.trim().is_empty())),
        due_date: Set(b.due_date),
        lead_days: Set(json!(lead_days)),
        recipients: Set(json!(recipients)),
        fired: Set(json!([])),
        status: Set("active".into()),
        completed_at: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REMINDER_CREATE,
        Some("reminder"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({
            "subject_type": saved.subject_type,
            "due_date": saved.due_date,
            "title": saved.title,
        })),
    )
    .await;

    Ok(Json(ReminderDto::from_model(saved, now.date_naive())))
}
