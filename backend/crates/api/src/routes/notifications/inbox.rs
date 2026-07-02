//! The signed-in user's **in-app inbox**: list, unread count, mark read.

use super::dto::InboxEntryDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Notification;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use uuid::Uuid;

/// `GET /notifications/inbox` — the signed-in user's in-app notifications,
/// newest first.
#[rocket_okapi::openapi(tag = "Notifications")]
#[get("/notifications/inbox?<limit>")]
pub async fn inbox(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    limit: Option<u64>,
) -> ApiResult<Json<Vec<InboxEntryDto>>> {
    let rows = Notification::find()
        .filter(entity::notification::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::notification::Column::UserId.eq(user.user_id))
        .filter(entity::notification::Column::Channel.eq("in_app"))
        .order_by_desc(entity::notification::Column::CreatedAt)
        .limit(limit.unwrap_or(50).min(200))
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(InboxEntryDto::from).collect()))
}

/// `GET /notifications/unread_count` — how many in-app notifications the
/// signed-in user hasn't read (powers the console bell badge).
#[rocket_okapi::openapi(tag = "Notifications")]
#[get("/notifications/unread_count")]
pub async fn unread_count(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<serde_json::Value>> {
    let unread = Notification::find()
        .filter(entity::notification::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::notification::Column::UserId.eq(user.user_id))
        .filter(entity::notification::Column::Channel.eq("in_app"))
        .filter(entity::notification::Column::ReadAt.is_null())
        .count(&db)
        .await?;
    Ok(Json(serde_json::json!({ "unread": unread })))
}

/// `POST /notifications/<id>/read` — mark one of your notifications read.
#[rocket_okapi::openapi(tag = "Notifications")]
#[post("/notifications/<id>/read")]
pub async fn mark_read(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<InboxEntryDto>> {
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let row = Notification::find_by_id(id)
        .filter(entity::notification::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::notification::Column::UserId.eq(user.user_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("notification not found".into()))?;
    if row.read_at.is_some() {
        return Ok(Json(InboxEntryDto::from(row)));
    }
    let mut am: entity::notification::ActiveModel = row.into();
    am.read_at = Set(Some(Utc::now().into()));
    am.updated_at = Set(Utc::now().into());
    Ok(Json(InboxEntryDto::from(am.update(&db).await?)))
}

/// `POST /notifications/read_all` — mark every unread notification read.
#[rocket_okapi::openapi(tag = "Notifications")]
#[post("/notifications/read_all")]
pub async fn mark_all_read(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<serde_json::Value>> {
    let now = Utc::now();
    let res = Notification::update_many()
        .col_expr(
            entity::notification::Column::ReadAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .col_expr(
            entity::notification::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(entity::notification::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::notification::Column::UserId.eq(user.user_id))
        .filter(entity::notification::Column::Channel.eq("in_app"))
        .filter(entity::notification::Column::ReadAt.is_null())
        .exec(&db)
        .await?;
    Ok(Json(serde_json::json!({ "marked": res.rows_affected })))
}
