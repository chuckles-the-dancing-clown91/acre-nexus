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
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::maintenance_ticket::Model> for TicketDto {
    fn from(t: entity::maintenance_ticket::Model) -> Self {
        TicketDto {
            cost_label: label(t.cost_cents),
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
