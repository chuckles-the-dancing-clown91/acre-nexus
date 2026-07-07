//! A **maintenance ticket** (work order) tracks a repair/turn task against a
//! property (optionally a specific unit/lease). It can be assigned either to a
//! platform user (a member) or to an external contractor in the entities registry
//! ([`super::counterparty`]).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "maintenance_ticket")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub lease_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    /// `plumbing` | `electrical` | `hvac` | `appliance` | `structural` | `general`.
    pub category: String,
    /// `low` | `normal` | `high` | `urgent`.
    pub priority: String,
    /// `open` | `triage` | `scheduled` | `in_progress` | `on_hold` | `resolved` | `closed`.
    pub status: String,
    /// Assigned platform user (a member), if any.
    pub assignee_user_id: Option<Uuid>,
    /// Assigned external contractor (counterparty), if any.
    pub assignee_entity_id: Option<Uuid>,
    /// Who reported the issue (free-form, e.g. the resident's name).
    pub reporter: Option<String>,
    /// Where in the home (e.g. "Kitchen", "Master bathroom").
    pub location: Option<String>,
    /// Entry instructions ("lockbox on rail", "dog in yard").
    pub access_notes: Option<String>,
    /// The resident authorized entry when they're not home.
    pub permission_to_enter: bool,
    /// The registered equipment being serviced ([`super::asset`]).
    pub asset_id: Option<Uuid>,
    pub due_date: Option<String>,
    /// Actual/estimated cost in cents.
    pub cost_cents: Option<i64>,
    /// When the first staff response landed (comment / triage / assignment).
    pub first_response_at: Option<DateTimeWithTimeZone>,
    /// When the ticket reached `resolved`/`closed` (cleared on reopen).
    pub resolved_at: Option<DateTimeWithTimeZone>,
    /// SLA targets stamped from the tenant's helpdesk policy at create (and
    /// re-stamped on priority change while the target is still open).
    pub sla_response_due_at: Option<DateTimeWithTimeZone>,
    pub sla_resolve_due_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
