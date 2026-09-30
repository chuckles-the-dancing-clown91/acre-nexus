//! A change to stock — `receive` | `use` | `count` | `restock` — with the unit cost it moved at.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_movement")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub inventory_item_id: Uuid,
    pub kind: String,
    /// Signed change.
    pub quantity: i32,
    pub unit_cost_cents: i64,
    pub ticket_id: Option<Uuid>,
    pub expense_id: Option<Uuid>,
    pub note: Option<String>,
    pub recorded_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
