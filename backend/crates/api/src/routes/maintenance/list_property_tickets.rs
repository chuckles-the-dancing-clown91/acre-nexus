use super::dto::TicketDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{MaintenanceTicket, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/tickets` — list a property's maintenance tickets, newest-first.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/properties/<id>/tickets")]
pub async fn list_property_tickets(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<TicketDto>>> {
    user.require(Permission::MaintenanceRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let rows = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_ticket::Column::PropertyId.eq(pid))
        .order_by_desc(entity::maintenance_ticket::Column::CreatedAt)
        .all(&state.property_db)
        .await?;
    Ok(Json(rows.into_iter().map(TicketDto::from).collect()))
}
