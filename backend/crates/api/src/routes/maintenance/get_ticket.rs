use super::dto::{TicketCommentDto, TicketDetailDto, TicketDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{MaintenanceTicket, TicketComment};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// `GET /tickets/<id>` — a maintenance ticket with its full comment timeline.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[get("/tickets/<id>")]
pub async fn get_ticket(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TicketDetailDto>> {
    user.require(Permission::MaintenanceRead)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let ticket = MaintenanceTicket::find_by_id(tid)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;
    let comments = TicketComment::find()
        .filter(entity::ticket_comment::Column::TicketId.eq(tid))
        .order_by_desc(entity::ticket_comment::Column::CreatedAt)
        .all(&db)
        .await?
        .into_iter()
        .map(TicketCommentDto::from)
        .collect();
    Ok(Json(TicketDetailDto {
        ticket: TicketDto::from(ticket),
        comments,
    }))
}
