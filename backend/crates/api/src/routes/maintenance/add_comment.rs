use super::dto::{AddCommentReq, TicketCommentDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::MaintenanceTicket;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /tickets/<id>/comments` — add a free-form comment to a ticket's timeline.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/tickets/<id>/comments", data = "<body>")]
pub async fn add_comment(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AddCommentReq>,
) -> ApiResult<Json<TicketCommentDto>> {
    user.require(Permission::MaintenanceManage)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    MaintenanceTicket::find_by_id(tid)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;
    let b = body.into_inner();
    let model = entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        ticket_id: Set(tid),
        author_user_id: Set(Some(user.user_id)),
        kind: Set("comment".to_string()),
        body: Set(b.body),
        created_at: Set(Utc::now().into()),
    };
    let saved = model.insert(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_COMMENT_ADD,
        Some("maintenance_ticket"),
        Some(tid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "comment_id": saved.id })),
    )
    .await;
    Ok(Json(TicketCommentDto::from(saved)))
}
