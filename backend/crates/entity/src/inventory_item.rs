//! An **inventory item** — parts/supplies stock the maintenance team draws
//! from: SKU, quantity on hand, unit cost, reorder level, storage location,
//! and a serial-number pool for serialized stock. Ticket lines
//! ([`super::ticket_line`]) consume it.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_item")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `NULL` = shared/company-wide stock; set = kept on site.
    pub property_id: Option<Uuid>,
    pub name: String,
    pub sku: Option<String>,
    /// `part` | `material` | `tool` | `supply` | `other`.
    pub category: String,
    pub quantity: i32,
    pub unit_cost_cents: Option<i64>,
    /// Alert when quantity falls to/below this (0 = never).
    pub reorder_level: i32,
    pub storage_location: Option<String>,
    /// Serial-number pool for serialized stock (JSON array of strings);
    /// consuming a part takes one out.
    pub serial_numbers: Json,
    pub notes: Option<String>,
    /// Set while an un-restocked low-stock alert is out; cleared (re-armed)
    /// once quantity rises back above the reorder level.
    pub low_stock_alerted_at: Option<DateTimeWithTimeZone>,
    /// `active` | `archived`.
    pub status: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
