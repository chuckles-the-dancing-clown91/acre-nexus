//! A **change order** on a [`crate::rehab_project`]: a signed delta to the
//! approved budget (positive for added scope, negative for a credit), gated by
//! an approval like vendor bills. An approved change order rolls into the
//! project's adjusted budget.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rehab_change_order")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    pub description: String,
    /// Signed change to the budget, in cents.
    pub amount_cents: i64,
    /// `pending` | `approved` | `rejected`.
    pub status: String,
    pub created_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub decided_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
