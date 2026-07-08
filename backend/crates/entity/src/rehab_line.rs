//! One **scope / budget line** on a [`crate::rehab_project`] — a category of work
//! with its budgeted cost. The sum of lines is the project's itemised budget.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rehab_line")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    /// Work category, e.g. `Roof`, `Kitchen`, `Electrical`.
    pub category: String,
    pub description: Option<String>,
    pub budget_cents: i64,
    pub sort_order: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
