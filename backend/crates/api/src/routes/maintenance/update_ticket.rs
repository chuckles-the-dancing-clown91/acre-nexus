use super::dto::{TicketDto, UpdateTicketReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Counterparty, MaintenanceTicket, Property, User};
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /tickets/<id>` — update fields on a maintenance ticket. A status
/// change is logged as a `status` comment on the timeline (and emailed to a
/// resident reporter); an assignment change dispatches the assignee (member
/// in-app + email, contractor by email); a priority change re-stamps the
/// still-open SLA targets; any staff touch counts as the first response.
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

    // Detect transitions before consuming the model.
    let status_changed = match &b.status {
        Some(s) if !s.is_empty() && *s != existing.status => Some(s.clone()),
        _ => None,
    };
    let priority_changed = match &b.priority {
        Some(p) if !p.is_empty() && *p != existing.priority => Some(p.clone()),
        _ => None,
    };
    let newly_assigned_user = b
        .assignee_user_id
        .filter(|v| existing.assignee_user_id != Some(*v));
    let newly_assigned_entity = b
        .assignee_entity_id
        .filter(|v| existing.assignee_entity_id != Some(*v));
    let had_first_response = existing.first_response_at.is_some();
    let created_at = existing.created_at;
    let was_resolved = existing.resolved_at.is_some();
    let property_id = existing.property_id;

    let now = Utc::now();
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
    if let Some(v) = b.location {
        am.location = Set(Some(v).filter(|s| !s.trim().is_empty()));
    }
    if let Some(v) = b.access_notes {
        am.access_notes = Set(Some(v).filter(|s| !s.trim().is_empty()));
    }
    if let Some(v) = b.permission_to_enter {
        am.permission_to_enter = Set(v);
    }
    if let Some(v) = b.asset_id {
        // Attach only equipment registered on this ticket's property.
        entity::prelude::Asset::find_by_id(v)
            .filter(entity::asset::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::asset::Column::PropertyId.eq(property_id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("asset not found on this property".into()))?;
        am.asset_id = Set(Some(v));
    }
    if let Some(v) = b.due_date {
        am.due_date = Set(Some(v));
    }
    if let Some(v) = b.cost_cents {
        am.cost_cents = Set(Some(v));
    }

    // Any staff touch that moves the ticket (status, assignment, triage)
    // counts as the first response.
    let responded = status_changed.is_some()
        || newly_assigned_user.is_some()
        || newly_assigned_entity.is_some();
    if !had_first_response && responded {
        am.first_response_at = Set(Some(now.into()));
    }

    // Resolution stamps on entering resolved/closed and clears on reopen.
    if let Some(new_status) = &status_changed {
        if matches!(new_status.as_str(), "resolved" | "closed") {
            if !was_resolved {
                am.resolved_at = Set(Some(now.into()));
            }
        } else if was_resolved {
            am.resolved_at = Set(None);
        }
    }

    // A priority change re-stamps the SLA targets that are still open,
    // measured from the ticket's creation.
    if let Some(new_priority) = &priority_changed {
        let (response_due, resolve_due) =
            crate::helpdesk::sla_targets(&db, scope.tenant_id, new_priority, created_at.to_utc())
                .await;
        if !had_first_response && !responded {
            am.first_response_at = Set(Some(now.into())); // triage is a response too
        }
        if !had_first_response {
            am.sla_response_due_at = Set(response_due.map(Into::into));
        }
        if !was_resolved {
            am.sla_resolve_due_at = Set(resolve_due.map(Into::into));
        }
    }

    am.updated_at = Set(now.into());
    let saved = am.update(&db).await?;

    // Log the status transition on the ticket timeline (best-effort).
    if let Some(new_status) = &status_changed {
        let comment = entity::ticket_comment::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            ticket_id: Set(saved.id),
            author_user_id: Set(Some(user.user_id)),
            kind: Set("status".to_string()),
            visibility: Set("public".into()),
            author_name: Set(None),
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

    // Contractor dispatch: assignment notifications (best-effort).
    if newly_assigned_user.is_some() || newly_assigned_entity.is_some() {
        let property = Property::find_by_id(saved.property_id)
            .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .map(|p| p.address)
            .unwrap_or_default();
        let due_line = saved
            .due_date
            .as_deref()
            .filter(|d| !d.is_empty())
            .map(|d| format!(", scheduled for {d}"))
            .unwrap_or_default();

        // A member assignee hears in-app + by email.
        if let Some(uid) = newly_assigned_user {
            if let Some(member) = User::find_by_id(uid).one(&db).await? {
                let vars = serde_json::json!({
                    "title": saved.title,
                    "priority": saved.priority,
                    "property": property,
                    "due_line": due_line,
                });
                crate::notify::in_app(
                    &db,
                    scope.tenant_id,
                    &member,
                    "ticket_assigned",
                    &vars,
                    Some(("maintenance_ticket", saved.id)),
                    &format!("assigned:{uid}"),
                )
                .await;
                let payload = serde_json::json!({
                    "template": "ticket_assigned",
                    "to": member.email,
                    "user_id": member.id,
                    "owner_type": "maintenance_ticket",
                    "owner_id": saved.id,
                    "trigger": format!("assigned_email:{uid}"),
                    "vars": vars,
                });
                if let Err(e) =
                    crate::scheduler::enqueue(&db, scope.tenant_id, "auto_email", payload, 0).await
                {
                    tracing::error!("failed to enqueue assignment email: {e}");
                }
            }
        }

        // An external contractor with an email on file gets the dispatch.
        if let Some(eid) = newly_assigned_entity {
            let contractor = Counterparty::find_by_id(eid)
                .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
                .one(&db)
                .await?;
            if let Some(email) = contractor
                .as_ref()
                .and_then(|c| c.email.as_deref())
                .filter(|e| !e.trim().is_empty())
            {
                let payload = serde_json::json!({
                    "template": "ticket_dispatch",
                    "to": email,
                    "owner_type": "maintenance_ticket",
                    "owner_id": saved.id,
                    "trigger": format!("dispatched:{eid}"),
                    "vars": {
                        "title": saved.title,
                        "priority": saved.priority,
                        "property": property,
                        "due_line": due_line,
                        "description": saved.description.clone().unwrap_or_default(),
                    },
                });
                if let Err(e) =
                    crate::scheduler::enqueue(&db, scope.tenant_id, "auto_email", payload, 0).await
                {
                    tracing::error!("failed to enqueue dispatch email: {e}");
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
