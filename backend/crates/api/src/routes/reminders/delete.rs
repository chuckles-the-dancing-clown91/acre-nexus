use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Reminder;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /reminders/<id>` — remove a reminder outright. Prefer cancelling
/// (PATCH `status: cancelled`) when the history matters.
#[rocket_okapi::openapi(tag = "Calendar")]
#[delete("/reminders/<id>")]
pub async fn delete_reminder(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::CalendarManage)?;
    let rid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let reminder = Reminder::find_by_id(rid)
        .filter(entity::reminder::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("reminder not found".into()))?;
    let title = reminder.title.clone();
    reminder.delete(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::REMINDER_DELETE,
        Some("reminder"),
        Some(rid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "title": title })),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}
