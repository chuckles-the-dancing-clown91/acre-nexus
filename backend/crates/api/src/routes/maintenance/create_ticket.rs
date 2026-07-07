use super::dto::{CreateTicketReq, TicketDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /properties/<id>/tickets` — open a new maintenance ticket on a property.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/properties/<id>/tickets", data = "<body>")]
pub async fn create_ticket(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateTicketReq>,
) -> ApiResult<Json<TicketDto>> {
    user.require(Permission::MaintenanceManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let b = body.into_inner();
    let now = Utc::now();
    let category = match b.category {
        Some(c) if !c.trim().is_empty() => c,
        _ => "general".to_string(),
    };
    let priority = match b.priority {
        Some(p) if !p.trim().is_empty() => p,
        _ => "normal".to_string(),
    };
    // Attach only equipment registered on this property.
    let asset_id = match b.asset_id {
        Some(aid) => {
            entity::prelude::Asset::find_by_id(aid)
                .filter(entity::asset::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::asset::Column::PropertyId.eq(pid))
                .one(&db)
                .await?
                .ok_or_else(|| ApiError::NotFound("asset not found on this property".into()))?;
            Some(aid)
        }
        None => None,
    };
    let (response_due, resolve_due) =
        crate::helpdesk::sla_targets(&db, scope.tenant_id, &priority, now).await;
    let model = entity::maintenance_ticket::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        unit_id: Set(b.unit_id),
        lease_id: Set(b.lease_id),
        title: Set(b.title),
        description: Set(b.description),
        category: Set(category),
        priority: Set(priority),
        status: Set("open".to_string()),
        assignee_user_id: Set(b.assignee_user_id),
        assignee_entity_id: Set(b.assignee_entity_id),
        reporter: Set(b.reporter),
        location: Set(b.location.filter(|s| !s.trim().is_empty())),
        access_notes: Set(b.access_notes.filter(|s| !s.trim().is_empty())),
        permission_to_enter: Set(b.permission_to_enter.unwrap_or(false)),
        asset_id: Set(asset_id),
        waiting_on: Set(None),
        follow_up_date: Set(None),
        rating: Set(None),
        review_comment: Set(None),
        reviewed_at: Set(None),
        due_date: Set(b.due_date),
        cost_cents: Set(b.cost_cents),
        first_response_at: Set(None),
        resolved_at: Set(None),
        sla_response_due_at: Set(response_due.map(Into::into)),
        sla_resolve_due_at: Set(resolve_due.map(Into::into)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = model.insert(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_CREATE,
        Some("maintenance_ticket"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": saved.property_id, "category": saved.category, "priority": saved.priority })),
    )
    .await;

    // Outbound webhooks (#68): subscribed vendors hear about new work orders.
    crate::webhooks_out::emit(
        &db,
        scope.tenant_id,
        "maintenance_ticket.created",
        serde_json::json!({
            "ticket_id": saved.id,
            "property_id": saved.property_id,
            "category": saved.category,
            "priority": saved.priority,
            "status": saved.status,
        }),
    )
    .await;

    // Integrated notifications: maintenance staff get an in-app entry + web
    // push (+ the workspace chat channel), except the actor who opened it.
    crate::notify::notify_staff(
        &db,
        scope.tenant_id,
        "maintenance:read",
        "ticket_created",
        serde_json::json!({ "title": saved.title, "priority": saved.priority }),
        Some(("maintenance_ticket", saved.id)),
        "created",
        Some(user.user_id),
    )
    .await;

    Ok(Json(TicketDto::from(saved)))
}
