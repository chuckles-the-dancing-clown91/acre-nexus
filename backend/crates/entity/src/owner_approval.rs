//! An **owner's say** on a work order: approval of work over their limit
//! before it starts, or sign-off on finished billable work. Answered from a
//! link in the email or text, or from the owner portal. One row per ask.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "owner_approval")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub owner_id: Uuid,
    pub ticket_id: Uuid,
    /// `approval` (before the work) | `signoff` (after it).
    pub kind: String,
    /// The estimate asked about, or the actual cost signed off.
    pub amount_cents: i64,
    /// `pending` | `approved` | `declined` | `disputed` | `overridden`.
    pub status: String,
    pub token_hash: Option<String>,
    /// What staff told the owner.
    pub note: Option<String>,
    pub requested_by: Option<Uuid>,
    pub requested_at: DateTimeWithTimeZone,
    pub decided_at: Option<DateTimeWithTimeZone>,
    /// `owner` | `staff`.
    pub decided_by: Option<String>,
    pub decision_note: Option<String>,
    /// Why staff went ahead without the owner (an emergency, say).
    pub override_reason: Option<String>,
    pub nudged_at: Option<DateTimeWithTimeZone>,
    pub nudges: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
