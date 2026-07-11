//! A **dues assessment** charged to an [`crate::hoa_member`] (issue #13) — a
//! recurring dues charge or a one-off special assessment. Money is integer cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "hoa_assessment")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `hoa_association.id`.
    pub association_id: Uuid,
    /// FK to `hoa_member.id`.
    pub member_id: Uuid,
    pub description: String,
    pub amount_cents: i64,
    /// Billing period label, e.g. "2026-07" (null for one-off special assessments).
    pub period: Option<String>,
    pub due_date: Option<String>,
    /// `due` | `paid` | `void`.
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
