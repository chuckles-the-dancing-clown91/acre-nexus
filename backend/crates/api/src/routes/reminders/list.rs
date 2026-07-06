use super::dto::ReminderDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Reminder;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

/// `GET /reminders?from=&to=&subject_type=&status=` — the schedule, soonest
/// first. `from`/`to` bound the due date (inclusive, `YYYY-MM-DD`); the
/// console calendar passes a month window.
#[rocket_okapi::openapi(tag = "Calendar")]
#[get("/reminders?<from>&<to>&<subject_type>&<status>")]
#[allow(clippy::too_many_arguments)]
pub async fn list_reminders(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    from: Option<&str>,
    to: Option<&str>,
    subject_type: Option<&str>,
    status: Option<&str>,
) -> ApiResult<Json<Vec<ReminderDto>>> {
    user.require(Permission::CalendarRead)?;
    let mut q = Reminder::find().filter(entity::reminder::Column::TenantId.eq(scope.tenant_id));
    if let Some(f) = from.filter(|s| !s.is_empty()) {
        q = q.filter(entity::reminder::Column::DueDate.gte(f));
    }
    if let Some(t) = to.filter(|s| !s.is_empty()) {
        q = q.filter(entity::reminder::Column::DueDate.lte(t));
    }
    if let Some(st) = subject_type.filter(|s| !s.is_empty()) {
        q = q.filter(entity::reminder::Column::SubjectType.eq(st));
    }
    if let Some(s) = status.filter(|s| !s.is_empty()) {
        q = q.filter(entity::reminder::Column::Status.eq(s));
    }
    let rows = q
        .order_by_asc(entity::reminder::Column::DueDate)
        .limit(500)
        .all(&db)
        .await?;
    let today = Utc::now().date_naive();
    Ok(Json(
        rows.into_iter()
            .map(|r| ReminderDto::from_model(r, today))
            .collect(),
    ))
}
