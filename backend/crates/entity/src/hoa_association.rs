//! An **HOA / community association** (issue #13 — HOA vertical): the governing
//! body for a community, optionally tied to a [`crate::property`]. Owns members,
//! dues assessments, violations, and architectural (ARC) requests. Money is
//! integer cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "hoa_association")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    /// Optional FK to `property.id` (the community this association governs).
    pub property_id: Option<Uuid>,
    /// Standard periodic dues per member, in cents.
    pub dues_cents: i64,
    /// `monthly` | `quarterly` | `annual`.
    pub dues_frequency: String,
    /// `active` | `inactive`.
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
