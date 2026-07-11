//! One investor's slice of a [`crate::capital_call`] (issue #13). The amount is
//! the investor's pro-rata share of the call by committed capital. Money is
//! integer cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "capital_call_line")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `capital_call.id`.
    pub call_id: Uuid,
    /// FK to `investor_commitment.id`.
    pub commitment_id: Uuid,
    /// FK to `owner.id` (denormalised for convenient display).
    pub owner_id: Uuid,
    /// This investor's called amount, in cents.
    pub amount_cents: i64,
    /// `pending` | `funded`.
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
