//! A **rehab / construction project** on a property (roadmap Phase 7, issue #40)
//! — the budget container for a flip/BRRRR renovation. Scope lives in
//! [`crate::rehab_line`], spend flows through [`crate::rehab_draw`], and
//! [`crate::rehab_change_order`]s adjust the approved budget. Money is integer
//! cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rehab_project")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub name: String,
    /// `planning` | `active` | `complete` | `on_hold`.
    pub status: String,
    /// Base scope budget, in cents (approved change orders adjust the total).
    pub budget_cents: i64,
    /// Contingency reserve, basis points of the base budget.
    pub contingency_bps: i32,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub notes: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
