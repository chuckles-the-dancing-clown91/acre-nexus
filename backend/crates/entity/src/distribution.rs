//! A **distribution** of cash to a legal entity's investors, run through the
//! three-tier waterfall (issue #13). The per-investor result is stored in
//! [`crate::distribution_line`]s. `pref_rate_bps` and `carry_bps` capture the
//! waterfall terms used for this event. Money is integer cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "distribution")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `llc.id`.
    pub entity_id: Uuid,
    /// Sequence number within the entity.
    pub number: i32,
    /// Total cash distributed, in cents.
    pub amount_cents: i64,
    /// Preferred-return rate applied (basis points).
    pub pref_rate_bps: i32,
    /// GP carried interest applied (basis points).
    pub carry_bps: i32,
    /// `final` (posted; capital returns applied to commitments).
    pub status: String,
    pub memo: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
