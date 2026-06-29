//! A row in a legal entity's **cap table**: one [`crate::owner`] holding a stake
//! in one [`crate::llc`] (legal entity). Ownership is stored in basis points
//! (`10000` = 100%) for exactness; `role` is `member` / `manager` / `investor`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "entity_ownership")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `llc.id` — the legal entity whose cap table this row belongs to.
    pub entity_id: Uuid,
    /// FK to `owner.id`.
    pub owner_id: Uuid,
    /// Ownership percentage in basis points (10000 = 100%).
    pub ownership_bps: i32,
    /// `member` | `manager` | `investor`.
    pub role: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
