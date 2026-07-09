//! An investor's **capital commitment** to a legal entity (issue #13 — investor /
//! syndication). Links an [`crate::owner`] to an [`crate::llc`] with a committed
//! amount; `contributed_cents` grows as capital calls are funded and
//! `returned_cents` grows as distributions return capital. Money is integer cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "investor_commitment")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `llc.id` — the legal entity / fund vehicle.
    pub entity_id: Uuid,
    /// FK to `owner.id` — the committing investor (or the GP/manager).
    pub owner_id: Uuid,
    /// `investor` (LP) | `manager` (GP, earns carry) | `member`.
    pub role: String,
    /// Total capital committed, in cents.
    pub committed_cents: i64,
    /// Capital actually funded so far (via funded capital calls), in cents.
    pub contributed_cents: i64,
    /// Contributed capital already returned by distributions, in cents.
    pub returned_cents: i64,
    /// `active` | `closed`.
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
