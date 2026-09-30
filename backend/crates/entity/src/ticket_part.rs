//! One part on a work order's parts list, from *potential* through the shopping list and close-out to *used*.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ticket_part")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub ticket_id: Uuid,
    pub inventory_item_id: Option<Uuid>,
    pub name: String,
    pub quantity: i32,
    /// `potential` | `needed` | `from_stock` | `to_order` | `ordered` | `pick_up` | `received` | `used` | `skipped`
    pub status: String,
    /// `asset` | `finding` | `plan` | `typed`
    pub source: String,
    pub finding_comment_id: Option<Uuid>,
    pub need_by: Option<String>,
    /// `property` | `office` | `other`
    pub ship_to: Option<String>,
    pub ship_to_note: Option<String>,
    pub vendor: Option<String>,
    pub tracking: Option<String>,
    pub unit_cost_cents: Option<i64>,
    pub expense_id: Option<Uuid>,
    pub ticket_line_id: Option<Uuid>,
    pub note: Option<String>,
    pub ordered_at: Option<DateTimeWithTimeZone>,
    pub ordered_by: Option<Uuid>,
    pub received_at: Option<DateTimeWithTimeZone>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
