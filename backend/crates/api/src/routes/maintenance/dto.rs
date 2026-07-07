//! Request/response shapes for the maintenance (work order) endpoints.

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Label an optional cents amount as USD.
fn label(cents: Option<i64>) -> Option<String> {
    cents.map(usd)
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TicketDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub lease_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub priority: String,
    pub status: String,
    pub assignee_user_id: Option<Uuid>,
    pub assignee_entity_id: Option<Uuid>,
    pub reporter: Option<String>,
    pub due_date: Option<String>,
    pub cost_cents: Option<i64>,
    pub cost_label: Option<String>,
    /// SLA / lifecycle timestamps (Phase 6).
    pub first_response_at: Option<String>,
    pub resolved_at: Option<String>,
    pub sla_response_due_at: Option<String>,
    pub sla_resolve_due_at: Option<String>,
    /// `none` | `on_track` | `met` | `breached`, derived at read time.
    pub sla_response_state: String,
    pub sla_resolve_state: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::maintenance_ticket::Model> for TicketDto {
    fn from(t: entity::maintenance_ticket::Model) -> Self {
        let now = chrono::Utc::now();
        let to_utc = |ts: &Option<chrono::DateTime<chrono::FixedOffset>>| ts.map(|v| v.to_utc());
        TicketDto {
            cost_label: label(t.cost_cents),
            sla_response_state: crate::helpdesk::sla_state(
                to_utc(&t.sla_response_due_at),
                to_utc(&t.first_response_at),
                now,
            )
            .to_string(),
            sla_resolve_state: crate::helpdesk::sla_state(
                to_utc(&t.sla_resolve_due_at),
                to_utc(&t.resolved_at),
                now,
            )
            .to_string(),
            id: t.id,
            tenant_id: t.tenant_id,
            property_id: t.property_id,
            unit_id: t.unit_id,
            lease_id: t.lease_id,
            title: t.title,
            description: t.description,
            category: t.category,
            priority: t.priority,
            status: t.status,
            assignee_user_id: t.assignee_user_id,
            assignee_entity_id: t.assignee_entity_id,
            reporter: t.reporter,
            due_date: t.due_date,
            cost_cents: t.cost_cents,
            first_response_at: t.first_response_at.map(|v| v.to_rfc3339()),
            resolved_at: t.resolved_at.map(|v| v.to_rfc3339()),
            sla_response_due_at: t.sla_response_due_at.map(|v| v.to_rfc3339()),
            sla_resolve_due_at: t.sla_resolve_due_at.map(|v| v.to_rfc3339()),
            created_at: t.created_at.to_rfc3339(),
            updated_at: t.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct TicketCommentDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub ticket_id: Uuid,
    pub author_user_id: Option<Uuid>,
    pub kind: String,
    pub body: String,
    pub created_at: String,
}

impl From<entity::ticket_comment::Model> for TicketCommentDto {
    fn from(c: entity::ticket_comment::Model) -> Self {
        TicketCommentDto {
            id: c.id,
            tenant_id: c.tenant_id,
            ticket_id: c.ticket_id,
            author_user_id: c.author_user_id,
            kind: c.kind,
            body: c.body,
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

/// A ticket plus its full comment timeline (newest-first).
#[derive(Serialize, schemars::JsonSchema)]
pub struct TicketDetailDto {
    #[serde(flatten)]
    pub ticket: TicketDto,
    pub comments: Vec<TicketCommentDto>,
    /// Contractor quotes on this work order (Phase 6).
    pub quotes: Vec<TicketQuoteDto>,
    /// The reply-to address that threads email back into this ticket's
    /// timeline (issue #62).
    pub inbound_email_address: Option<String>,
}

/// A contractor's quote on a work order.
#[derive(Serialize, schemars::JsonSchema)]
pub struct TicketQuoteDto {
    pub id: Uuid,
    pub ticket_id: Uuid,
    pub entity_id: Uuid,
    /// Contractor display name, resolved for lists.
    pub entity_name: Option<String>,
    pub description: String,
    pub amount_cents: i64,
    pub amount_label: String,
    /// `pending` | `approved` | `rejected`.
    pub status: String,
    pub decided_at: Option<String>,
    pub created_at: String,
}

impl TicketQuoteDto {
    pub fn from_model(q: entity::ticket_quote::Model, entity_name: Option<String>) -> Self {
        TicketQuoteDto {
            id: q.id,
            ticket_id: q.ticket_id,
            entity_id: q.entity_id,
            entity_name,
            amount_label: usd(q.amount_cents),
            description: q.description,
            amount_cents: q.amount_cents,
            status: q.status,
            decided_at: q.decided_at.map(|v| v.to_rfc3339()),
            created_at: q.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateQuoteReq {
    /// Contractor (counterparty). Defaults to the ticket's assigned contractor.
    pub entity_id: Option<Uuid>,
    pub description: String,
    pub amount_cents: i64,
}

/// A preventive-maintenance plan.
#[derive(Serialize, schemars::JsonSchema)]
pub struct MaintenancePlanDto {
    pub id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub priority: String,
    pub cadence_days: i32,
    pub next_due_date: String,
    pub active: bool,
    pub last_ticket_id: Option<Uuid>,
    pub created_at: String,
}

impl From<entity::maintenance_plan::Model> for MaintenancePlanDto {
    fn from(p: entity::maintenance_plan::Model) -> Self {
        MaintenancePlanDto {
            id: p.id,
            property_id: p.property_id,
            unit_id: p.unit_id,
            title: p.title,
            description: p.description,
            category: p.category,
            priority: p.priority,
            cadence_days: p.cadence_days,
            next_due_date: p.next_due_date,
            active: p.active,
            last_ticket_id: p.last_ticket_id,
            created_at: p.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreatePlanReq {
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub cadence_days: i32,
    /// ISO date the first ticket opens.
    pub next_due_date: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdatePlanReq {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub cadence_days: Option<i32>,
    pub next_due_date: Option<String>,
    pub active: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateTicketReq {
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub unit_id: Option<Uuid>,
    pub lease_id: Option<Uuid>,
    pub assignee_user_id: Option<Uuid>,
    pub assignee_entity_id: Option<Uuid>,
    pub reporter: Option<String>,
    pub due_date: Option<String>,
    pub cost_cents: Option<i64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateTicketReq {
    pub title: Option<String>,
    pub description: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub assignee_user_id: Option<Uuid>,
    pub assignee_entity_id: Option<Uuid>,
    pub reporter: Option<String>,
    pub due_date: Option<String>,
    pub cost_cents: Option<i64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddCommentReq {
    pub body: String,
}

/// The Maintenance tab for a property: open work orders split from resolved
/// history, plus roll-up counts and the cost of open work.
#[derive(Serialize, schemars::JsonSchema)]
pub struct PropertyMaintenanceResp {
    pub property_id: Uuid,
    pub total_count: i64,
    pub open_count: i64,
    /// Sum of recorded cost on open tickets, in cents.
    pub open_cost_cents: i64,
    pub open_cost_label: String,
    /// Open/active work orders, newest first.
    pub open: Vec<TicketDto>,
    /// Resolved/closed tickets — the maintenance history, newest first.
    pub history: Vec<TicketDto>,
}
