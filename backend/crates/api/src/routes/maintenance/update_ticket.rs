use super::dto::{TicketDto, UpdateTicketReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::MaintenanceTicket;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /tickets/<id>` — update fields on a maintenance ticket. A status change
/// is also logged as a `status` comment on the ticket timeline.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[patch("/tickets/<id>", data = "<body>")]
pub async fn update_ticket(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateTicketReq>,
) -> ApiResult<Json<TicketDto>> {
    user.require(Permission::MaintenanceManage)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = MaintenanceTicket::find_by_id(tid)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;
    let b = body.into_inner();

    // Detect a status transition before consuming the model.
    let status_changed = match &b.status {
        Some(s) if !s.is_empty() && *s != existing.status => Some(s.clone()),
        _ => None,
    };

    let mut am: entity::maintenance_ticket::ActiveModel = existing.into();
    if let Some(v) = b.title {
        am.title = Set(v);
    }
    if let Some(v) = b.description {
        am.description = Set(Some(v));
    }
    if let Some(v) = b.category {
        am.category = Set(v);
    }
    if let Some(v) = b.priority {
        am.priority = Set(v);
    }
    if let Some(v) = b.status {
        am.status = Set(v);
    }
    if let Some(v) = b.assignee_user_id {
        am.assignee_user_id = Set(Some(v));
    }
    if let Some(v) = b.assignee_entity_id {
        am.assignee_entity_id = Set(Some(v));
    }
    if let Some(v) = b.reporter {
        am.reporter = Set(Some(v));
    }
    if let Some(v) = b.due_date {
        am.due_date = Set(Some(v));
    }
    if let Some(v) = b.cost_cents {
        am.cost_cents = Set(Some(v));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    // Log the status transition on the ticket timeline (best-effort).
    if let Some(new_status) = &status_changed {
        let comment = entity::ticket_comment::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            ticket_id: Set(saved.id),
            author_user_id: Set(Some(user.user_id)),
            kind: Set("status".to_string()),
            body: Set(format!("Status -> {}", new_status)),
            created_at: Set(Utc::now().into()),
        };
        if let Err(e) = comment.insert(&db).await {
            tracing::error!("failed to log status comment: {e}");
        }

        // A resident-reported request emails the resident on every status
        // move, so the portal round-trips (best-effort).
        if let Some(lease_id) = saved.lease_id {
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
                    "template": "maintenance_update",
                    "to": email,
                    "owner_type": "maintenance_ticket",
                    "owner_id": saved.id,
                    "trigger": format!("status:{new_status}"),
                    "vars": {
                        "title": saved.title,
                        "status": new_status.replace('_', " "),
                    },
                });
                if let Err(e) =
                    crate::scheduler::enqueue(&db, scope.tenant_id, "auto_email", payload, 0).await
                {
                    tracing::error!("failed to enqueue maintenance update email: {e}");
                }
            }
        }
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_UPDATE,
        Some("maintenance_ticket"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status, "priority": saved.priority })),
    )
    .await;
    Ok(Json(TicketDto::from(saved)))
}
