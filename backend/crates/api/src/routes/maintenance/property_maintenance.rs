//! `GET /properties/<id>/maintenance` — the Maintenance tab: open work orders,
//! resolved history, and roll-up counts.

use super::dto::{PropertyMaintenanceResp, TicketDto};
use super::is_open;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{MaintenanceTicket, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /properties/<id>/maintenance` — split a property's tickets into open
/// work vs resolved history, with counts and the cost of open work.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/properties/<id>/maintenance")]
pub async fn property_maintenance(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PropertyMaintenanceResp>> {
    user.require(Permission::MaintenanceRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let rows = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_ticket::Column::PropertyId.eq(pid))
        .order_by_desc(entity::maintenance_ticket::Column::CreatedAt)
        .all(&db)
        .await?;

    let total_count = rows.len() as i64;
    let mut open: Vec<TicketDto> = Vec::new();
    let mut history: Vec<TicketDto> = Vec::new();
    let mut open_cost_cents: i64 = 0;
    for t in rows {
        if is_open(&t.status) {
            open_cost_cents += t.cost_cents.unwrap_or(0);
            open.push(TicketDto::from(t));
        } else {
            history.push(TicketDto::from(t));
        }
    }

    Ok(Json(PropertyMaintenanceResp {
        property_id: pid,
        total_count,
        open_count: open.len() as i64,
        open_cost_cents,
        open_cost_label: usd(open_cost_cents),
        open,
        history,
    }))
}
