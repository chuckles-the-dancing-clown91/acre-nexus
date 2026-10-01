//! An **issue template**: a common problem in the catalog, with the category,
//! priority, checklist and usual parts that "generate ticket" fills in.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "issue_template")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    /// Where in the home (e.g. "Kitchen").
    pub area: Option<String>,
    pub category: String,
    pub priority: String,
    pub description: Option<String>,
    pub est_minutes: Option<i32>,
    /// JSON array of checklist lines.
    pub checklist: Json,
    /// JSON array of `{ name, quantity, inventory_item_id? }`.
    pub parts: Json,
    pub active: bool,
    /// Created from the built-in starter set (kept editable).
    pub seeded: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
