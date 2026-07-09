//! A **capital call** against a legal entity's investors (issue #13 — investor /
//! syndication). Calls a total amount, split pro-rata by committed capital into
//! [`crate::capital_call_line`]s; funding the call credits each investor's
//! contributed capital. Money is integer cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "capital_call")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `llc.id`.
    pub entity_id: Uuid,
    /// Sequence number within the entity (1, 2, 3 …).
    pub number: i32,
    /// Total capital called, in cents.
    pub amount_cents: i64,
    /// `open` | `funded`.
    pub status: String,
    pub due_date: Option<String>,
    pub memo: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
