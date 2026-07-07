//! A **ticket line** — one itemized part / labor / fee entry on a
//! [`super::maintenance_ticket`]; line totals drive the ticket's cost. A
//! `part` line can consume [`super::inventory_item`] stock (and a serial),
//! and restocks if removed.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ticket_line")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub ticket_id: Uuid,
    /// `part` | `labor` | `fee` | `other`.
    pub kind: String,
    pub description: String,
    /// Set when the part came out of inventory.
    pub inventory_item_id: Option<Uuid>,
    pub serial_number: Option<String>,
    pub quantity: i32,
    pub unit_cost_cents: i64,
    pub total_cents: i64,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
