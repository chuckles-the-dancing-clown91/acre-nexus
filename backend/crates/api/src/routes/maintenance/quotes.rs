//! Contractor quotes on a work order (Phase 6): record a quote against a
//! ticket (`maintenance:manage`), then approve or reject it with the same
//! permission that approves vendor bills (`payable:approve`). Approval
//! lands the quoted amount as a labor line — lines are the single source of
//! the ticket's cost — so the existing ticket → vendor-bill prefill carries
//! it straight into accounts payable.

use super::dto::{CreateQuoteReq, TicketQuoteDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Counterparty, MaintenanceTicket, TicketQuote};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use std::collections::HashMap;
use uuid::Uuid;

/// All quotes on one ticket, newest first, with contractor names resolved.
pub async fn quotes_for_ticket(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    ticket_id: Uuid,
) -> ApiResult<Vec<TicketQuoteDto>> {
    let quotes = TicketQuote::find()
        .filter(entity::ticket_quote::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_quote::Column::TicketId.eq(ticket_id))
        .order_by_desc(entity::ticket_quote::Column::CreatedAt)
        .all(db)
        .await?;
    let entity_ids: Vec<Uuid> = quotes.iter().map(|q| q.entity_id).collect();
    let names: HashMap<Uuid, String> = Counterparty::find()
        .filter(entity::counterparty::Column::TenantId.eq(tenant_id))
        .filter(entity::counterparty::Column::Id.is_in(entity_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect();
    Ok(quotes
        .into_iter()
        .map(|q| {
            let name = names.get(&q.entity_id).cloned();
            TicketQuoteDto::from_model(q, name)
        })
        .collect())
}

/// A tenant-scoped ticket, or 404 (shared across the ticket sub-routes).
pub(super) async fn find_ticket(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::maintenance_ticket::Model> {
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    MaintenanceTicket::find_by_id(tid)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))
}

/// `POST /tickets/<id>/quotes` — record a contractor's quote on a work
/// order. The contractor defaults to the ticket's assigned contractor.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/tickets/<id>/quotes", data = "<body>")]
pub async fn add_quote(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateQuoteReq>,
) -> ApiResult<Json<TicketQuoteDto>> {
    user.require(Permission::MaintenanceManage)?;
    let ticket = find_ticket(&db, scope.tenant_id, id).await?;
    let b = body.into_inner();
    if b.amount_cents <= 0 {
        return Err(ApiError::BadRequest("quote amount must be positive".into()));
    }
    let description = b.description.trim().to_string();
    if description.is_empty() {
        return Err(ApiError::BadRequest("description is required".into()));
    }
    let entity_id = b.entity_id.or(ticket.assignee_entity_id).ok_or_else(|| {
        ApiError::BadRequest("pass entity_id or assign a contractor to the ticket first".into())
    })?;
    let contractor = Counterparty::find_by_id(entity_id)
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("contractor not found".into()))?;

    let saved = entity::ticket_quote::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        ticket_id: Set(ticket.id),
        entity_id: Set(entity_id),
        description: Set(description),
        amount_cents: Set(b.amount_cents),
        status: Set("pending".into()),
        decided_by: Set(None),
        decided_at: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_QUOTE_ADD,
        Some("ticket_quote"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "ticket_id": ticket.id,
            "entity_id": entity_id,
            "amount_cents": saved.amount_cents,
        })),
    )
    .await;

    Ok(Json(TicketQuoteDto::from_model(
        saved,
        Some(contractor.name),
    )))
}

/// Approve or reject one pending quote.
async fn decide(
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    approve: bool,
) -> ApiResult<Json<TicketQuoteDto>> {
    user.require(Permission::PayableApprove)?;
    let qid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let quote = TicketQuote::find_by_id(qid)
        .filter(entity::ticket_quote::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("quote not found".into()))?;
    if quote.status != "pending" {
        return Err(ApiError::BadRequest(format!(
            "quote is already {}",
            quote.status
        )));
    }
    let ticket = MaintenanceTicket::find_by_id(quote.ticket_id)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;

    let now = Utc::now();
    let amount = quote.amount_cents;
    let entity_id = quote.entity_id;
    let mut am: entity::ticket_quote::ActiveModel = quote.into();
    am.status = Set(if approve { "approved" } else { "rejected" }.into());
    am.decided_by = Set(Some(user.user_id));
    am.decided_at = Set(Some(now.into()));
    let saved = am.update(&db).await?;

    // Approval feeds the ticket: the quoted amount lands as a labor line, so
    // the itemized total — the single source of the ticket's cost, which the
    // vendor-bill prefill reads — always includes the contractor's work
    // (and survives parts being added or removed around it). The contractor
    // is attached if the ticket had none.
    if approve {
        if ticket.assignee_entity_id.is_none() {
            let mut tam: entity::maintenance_ticket::ActiveModel = ticket.clone().into();
            tam.assignee_entity_id = Set(Some(entity_id));
            tam.updated_at = Set(now.into());
            tam.update(&db).await?;
        }
        entity::ticket_line::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            ticket_id: Set(ticket.id),
            kind: Set("labor".into()),
            description: Set(format!("Approved quote: {}", saved.description)),
            inventory_item_id: Set(None),
            serial_number: Set(None),
            quantity: Set(1),
            unit_cost_cents: Set(amount),
            total_cents: Set(amount),
            created_by: Set(Some(user.user_id)),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await?;
        let ticket = MaintenanceTicket::find_by_id(ticket.id)
            .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;
        super::lines::sync_ticket_cost(&db, scope.tenant_id, ticket).await?;
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        if approve {
            crate::audit::actions::TICKET_QUOTE_APPROVE
        } else {
            crate::audit::actions::TICKET_QUOTE_REJECT
        },
        Some("ticket_quote"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "ticket_id": saved.ticket_id,
            "amount_cents": amount,
        })),
    )
    .await;

    let name = Counterparty::find_by_id(entity_id)
        .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .map(|c| c.name);
    Ok(Json(TicketQuoteDto::from_model(saved, name)))
}

/// `POST /ticket-quotes/<id>/approve` — approve a pending quote
/// (`payable:approve`, the same gate as vendor bills).
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/ticket-quotes/<id>/approve")]
pub async fn approve_quote(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TicketQuoteDto>> {
    decide(db, user, scope, id, true).await
}

/// `POST /ticket-quotes/<id>/reject` — reject a pending quote.
#[rocket_okapi::openapi(tag = "Maintenance")]
#[post("/ticket-quotes/<id>/reject")]
pub async fn reject_quote(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<TicketQuoteDto>> {
    decide(db, user, scope, id, false).await
}
