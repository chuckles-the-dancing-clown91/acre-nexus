//! **Ticket lines** — itemized parts / labor / fees on a work order. Line
//! totals drive the ticket's cost (which the vendor-bill prefill reads). A
//! `part` line can pull from inventory: stock is validated + decremented
//! (and a serial consumed from the pool); removing the line restocks.

use super::dto::{serials_from_json, CreateLineReq, TicketLineDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{InventoryItem, MaintenanceTicket, TicketLine};
use rocket::serde::json::Json;
use rocket::{delete, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect,
    Set,
};
use uuid::Uuid;

/// Line kinds the API accepts.
pub const KINDS: &[&str] = &["part", "labor", "fee", "other"];

/// All lines for one ticket, oldest first.
pub async fn lines_for_ticket(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    ticket_id: Uuid,
) -> ApiResult<Vec<TicketLineDto>> {
    Ok(TicketLine::find()
        .filter(entity::ticket_line::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_line::Column::TicketId.eq(ticket_id))
        .order_by_asc(entity::ticket_line::Column::CreatedAt)
        .all(db)
        .await?
        .into_iter()
        .map(TicketLineDto::from)
        .collect())
}

/// Recompute the ticket's cost from its line totals. Quote approval also
/// calls this: an approved quote lands as a line, so the itemized total is
/// the single source of truth whenever any lines exist.
pub(super) async fn sync_ticket_cost(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    ticket: entity::maintenance_ticket::Model,
) -> ApiResult<()> {
    let lines = TicketLine::find()
        .filter(entity::ticket_line::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_line::Column::TicketId.eq(ticket.id))
        .all(db)
        .await?;
    let total: i64 = lines.iter().map(|l| l.total_cents).sum();
    let mut am: entity::maintenance_ticket::ActiveModel = ticket.into();
    am.cost_cents = Set(if lines.is_empty() { None } else { Some(total) });
    am.updated_at = Set(Utc::now().into());
    am.update(db).await?;
    Ok(())
}

/// `POST /tickets/<id>/lines` — add a part / labor / fee line. A part pulled
/// from inventory validates + decrements stock (and consumes the serial).
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/tickets/<id>/lines", data = "<body>")]
pub async fn add_line(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateLineReq>,
) -> ApiResult<Json<TicketLineDto>> {
    user.require(Permission::MaintenanceManage)?;
    let ticket = super::quotes::find_ticket(&db, scope.tenant_id, id).await?;
    let tid = ticket.id;
    let b = body.into_inner();

    let kind = match b.kind.as_deref().map(str::trim) {
        None | Some("") => "part".to_string(),
        Some(k) if KINDS.contains(&k) => k.to_string(),
        Some(k) => {
            return Err(ApiError::BadRequest(format!(
                "invalid kind: {k} (expected one of {})",
                KINDS.join(", ")
            )))
        }
    };
    let quantity = b.quantity.unwrap_or(1);
    if quantity <= 0 {
        return Err(ApiError::BadRequest("quantity must be positive".into()));
    }

    // Pulling from inventory: validate stock, consume the serial, decrement.
    let mut description = b.description.map(|s| s.trim().to_string());
    let mut unit_cost = b.unit_cost_cents;
    let serial = b
        .serial_number
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(item_id) = b.inventory_item_id {
        // FOR UPDATE: the stock check, serial consume, and decrement below
        // are read-modify-write — without the row lock two concurrent pulls
        // both pass the check and oversell (or take the same serial).
        let item = InventoryItem::find_by_id(item_id)
            .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::inventory_item::Column::Status.eq("active"))
            .lock_exclusive()
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("inventory item not found".into()))?;
        if item.quantity < quantity {
            return Err(ApiError::BadRequest(format!(
                "not enough stock: {} on hand, {} requested",
                item.quantity, quantity
            )));
        }
        let mut serials = serials_from_json(&item.serial_numbers);
        if let Some(sn) = &serial {
            if quantity != 1 {
                return Err(ApiError::BadRequest(
                    "a serialized line uses quantity 1 (one serial per line)".into(),
                ));
            }
            let before = serials.len();
            serials.retain(|s| s != sn);
            if serials.len() == before {
                return Err(ApiError::NotFound(
                    "serial not found in the item's pool".into(),
                ));
            }
        } else if !serials.is_empty() {
            return Err(ApiError::BadRequest(
                "this item is serialized — pass serial_number to consume a unit".into(),
            ));
        }
        description = description
            .filter(|d| !d.is_empty())
            .or_else(|| Some(item.name.clone()));
        if unit_cost.is_none() {
            unit_cost = item.unit_cost_cents;
        }
        let next_quantity = item.quantity - quantity;
        let mut iam: entity::inventory_item::ActiveModel = item.into();
        iam.quantity = Set(next_quantity);
        iam.serial_numbers = Set(serde_json::json!(serials));
        iam.updated_at = Set(Utc::now().into());
        iam.update(&db).await?;
    }

    let description = description
        .filter(|d| !d.is_empty())
        .ok_or_else(|| ApiError::BadRequest("description is required".into()))?;
    let unit_cost = unit_cost.unwrap_or(0);
    if unit_cost < 0 {
        return Err(ApiError::BadRequest("unit cost cannot be negative".into()));
    }
    let total = unit_cost
        .checked_mul(i64::from(quantity))
        .ok_or_else(|| ApiError::BadRequest("line total is too large".into()))?;

    let saved = entity::ticket_line::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        ticket_id: Set(ticket.id),
        kind: Set(kind),
        description: Set(description),
        inventory_item_id: Set(b.inventory_item_id),
        serial_number: Set(serial),
        quantity: Set(quantity),
        unit_cost_cents: Set(unit_cost),
        total_cents: Set(total),
        created_by: Set(Some(user.user_id)),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    sync_ticket_cost(&db, scope.tenant_id, ticket).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_LINE_ADD,
        Some("ticket_line"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "ticket_id": tid,
            "kind": saved.kind,
            "total_cents": saved.total_cents,
            "inventory_item_id": saved.inventory_item_id,
        })),
    )
    .await;

    Ok(Json(TicketLineDto::from(saved)))
}

/// `DELETE /ticket-lines/<id>` — remove a line; an inventory-backed part
/// restocks (and its serial returns to the pool).
#[rocket_okapi::openapi(tag = "Maintenance")]
#[delete("/ticket-lines/<id>")]
pub async fn remove_line(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::MaintenanceManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let line = TicketLine::find_by_id(lid)
        .filter(entity::ticket_line::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("line not found".into()))?;
    let ticket = MaintenanceTicket::find_by_id(line.ticket_id)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;

    // Delete FIRST and only restock if this request actually removed the
    // row — a double-submitted DELETE must not restock (and re-pool the
    // serial) twice.
    let line_id = line.id;
    let ticket_id = line.ticket_id;
    let deleted = line.clone().delete(&db).await?;
    if deleted.rows_affected == 0 {
        return Err(ApiError::NotFound("line not found".into()));
    }

    // Restock an inventory-backed part (FOR UPDATE — see add_line).
    if let Some(item_id) = line.inventory_item_id {
        if let Some(item) = InventoryItem::find_by_id(item_id)
            .filter(entity::inventory_item::Column::TenantId.eq(scope.tenant_id))
            .lock_exclusive()
            .one(&db)
            .await?
        {
            let mut serials = serials_from_json(&item.serial_numbers);
            if let Some(sn) = &line.serial_number {
                serials.push(sn.clone());
            }
            let next_quantity = item.quantity + line.quantity;
            let mut iam: entity::inventory_item::ActiveModel = item.into();
            iam.quantity = Set(next_quantity);
            iam.serial_numbers = Set(serde_json::json!(serials));
            iam.updated_at = Set(Utc::now().into());
            iam.update(&db).await?;
        }
    }

    sync_ticket_cost(&db, scope.tenant_id, ticket).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_LINE_REMOVE,
        Some("ticket_line"),
        Some(line_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "ticket_id": ticket_id })),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true })))
}
