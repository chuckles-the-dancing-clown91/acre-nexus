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
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateTicketReq>,
) -> ApiResult<Json<TicketDto>> {
    user.require(Permission::MaintenanceManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
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
        due_date: Set(b.due_date),
        cost_cents: Set(b.cost_cents),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = model.insert(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::TICKET_CREATE,
        Some("maintenance_ticket"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": saved.property_id, "category": saved.category, "priority": saved.priority })),
    )
    .await;
    Ok(Json(TicketDto::from(saved)))
}
