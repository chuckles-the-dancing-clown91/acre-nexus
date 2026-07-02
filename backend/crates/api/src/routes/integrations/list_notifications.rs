//! `GET /integrations/notifications` — the outbound send history.

use super::dto::NotificationDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Notification;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

/// `GET /integrations/notifications` — the most recent outbound notifications
/// (email + SMS) with delivery status.
#[rocket_okapi::openapi(tag = "Integrations")]
#[get("/integrations/notifications?<limit>")]
pub async fn list_notifications(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    limit: Option<u64>,
) -> ApiResult<Json<Vec<NotificationDto>>> {
    user.require(Permission::IntegrationsManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;

    let rows = Notification::find()
        .filter(entity::notification::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::notification::Column::CreatedAt)
        .limit(limit.unwrap_or(100).min(500))
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(NotificationDto::from).collect()))
}
