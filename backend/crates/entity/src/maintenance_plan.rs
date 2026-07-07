//! A **preventive-maintenance plan** — a recurring task (HVAC service,
//! gutter cleaning, smoke-detector checks) that auto-opens a
//! [`super::maintenance_ticket`] every `cadence_days`, driven by the
//! helpdesk scan job.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "maintenance_plan")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    /// Ticket category the plan generates.
    pub category: String,
    /// Ticket priority the plan generates.
    pub priority: String,
    /// How often a ticket is generated.
    pub cadence_days: i32,
    /// ISO date (`YYYY-MM-DD`) the next ticket opens.
    pub next_due_date: String,
    pub active: bool,
    /// The most recent auto-opened ticket.
    pub last_ticket_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
