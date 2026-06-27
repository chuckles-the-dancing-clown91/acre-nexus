use super::dto::TicketDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::MaintenanceTicket;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /tickets?<status>&<property_id>&<priority>` — list the active tenant's
/// maintenance tickets, optionally filtered, newest-first.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/tickets?<status>&<property_id>&<priority>")]
pub async fn list_tickets(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    status: Option<String>,
    property_id: Option<String>,
    priority: Option<String>,
) -> ApiResult<Json<Vec<TicketDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let mut q = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.is_empty()) {
        q = q.filter(entity::maintenance_ticket::Column::Status.eq(s));
    }
    if let Some(p) = priority.filter(|s| !s.is_empty()) {
        q = q.filter(entity::maintenance_ticket::Column::Priority.eq(p));
    }
    if let Some(pid) = property_id.filter(|s| !s.is_empty()) {
        let pid = Uuid::parse_str(&pid).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
        q = q.filter(entity::maintenance_ticket::Column::PropertyId.eq(pid));
    }
    let rows = q
        .order_by_desc(entity::maintenance_ticket::Column::CreatedAt)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(TicketDto::from).collect()))
}
