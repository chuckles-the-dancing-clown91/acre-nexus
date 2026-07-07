use super::dto::{AddCommentReq, TicketCommentDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{MaintenanceTicket, User};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /tickets/<id>/comments` — add to a ticket's timeline: a **public
/// reply** (default — the resident sees it in their portal and is emailed)
/// or an **internal note** (`visibility: "internal"`, staff-only).
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
    let ticket = MaintenanceTicket::find_by_id(tid)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;
    let b = body.into_inner();
    let visibility = match b.visibility.as_deref().map(str::trim) {
        None | Some("") | Some("public") => "public",
        Some("internal") => "internal",
        Some(v) => {
            return Err(ApiError::BadRequest(format!(
                "invalid visibility: {v} (expected public|internal)"
            )))
        }
    };
    let text = b.body.trim().to_string();
    if text.is_empty() {
        return Err(ApiError::BadRequest("comment body is required".into()));
    }

    // A staff comment is the first response when none is recorded yet.
    if ticket.first_response_at.is_none() {
        let mut am: entity::maintenance_ticket::ActiveModel = ticket.clone().into();
        am.first_response_at = Set(Some(Utc::now().into()));
        am.update(&db).await?;
    }

    let author_name = User::find_by_id(user.user_id).one(&db).await?.map(|u| {
        if u.name.trim().is_empty() {
            u.email
        } else {
            u.name
        }
    });

    let model = entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        ticket_id: Set(tid),
        author_user_id: Set(Some(user.user_id)),
        kind: Set("comment".to_string()),
        visibility: Set(visibility.to_string()),
        author_name: Set(author_name),
        body: Set(text),
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
        Some(serde_json::json!({ "comment_id": saved.id, "visibility": visibility })),
    )
    .await;

    // A public reply on a resident-reported request emails the resident, so
    // the conversation round-trips (best-effort). Internal notes never do.
    if visibility == "public" {
        if let Some(lease_id) = ticket.lease_id {
            let lease = entity::prelude::Lease::find_by_id(lease_id)
                .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
                .one(&db)
                .await?;
            if let Some(email) = lease
                .as_ref()
                .and_then(|l| l.tenant_email.as_deref())
                .filter(|e| !e.trim().is_empty())
            {
                let payload = serde_json::json!({
                    "template": "maintenance_reply",
                    "to": email,
                    "owner_type": "maintenance_ticket",
                    "owner_id": ticket.id,
                    "trigger": format!("reply:{}", saved.id),
                    "vars": {
                        "title": ticket.title,
                        "author": saved.author_name.clone().unwrap_or_else(|| "the team".into()),
                        "preview": crate::routes::messages::preview(&saved.body),
                    },
                });
                if let Err(e) =
                    crate::scheduler::enqueue(&db, scope.tenant_id, "auto_email", payload, 0).await
                {
                    tracing::error!("failed to enqueue maintenance reply email: {e}");
                }
            }
        }
    }

    Ok(Json(TicketCommentDto::from(saved)))
}
