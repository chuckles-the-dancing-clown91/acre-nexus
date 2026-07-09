//! One investor's slice of a [`crate::distribution`], broken out by waterfall
//! tier (issue #13). `total_cents` is the sum of the tiers. Money is integer
//! cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "distribution_line")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `distribution.id`.
    pub distribution_id: Uuid,
    /// FK to `investor_commitment.id`.
    pub commitment_id: Uuid,
    /// FK to `owner.id`.
    pub owner_id: Uuid,
    /// Tier 1 — return of capital.
    pub return_of_capital_cents: i64,
    /// Tier 2 — preferred return.
    pub preferred_cents: i64,
    /// Tier 3 — post-carry profit share (LP).
    pub profit_cents: i64,
    /// Tier 3 — carried interest (GP).
    pub carry_cents: i64,
    /// Sum of the tiers — what this investor receives.
    pub total_cents: i64,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
