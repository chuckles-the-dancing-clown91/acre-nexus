//! A **fee schedule** entry is a landlord-configured, reusable fee / discount /
//! rebate / amenity. Each carries a **condition** that decides when it auto-applies
//! to a lease (e.g. `has_pet` → a pet fee, `is_military` → a discount, `has_vehicle`
//! → a garage amenity) plus optional lease `verbiage` that is woven into the
//! generated lease document. `manual` entries are offered but never auto-applied.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "fee_schedule")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// Stable per-tenant code, e.g. `pet_fee`, `military_discount`, `garage`.
    pub code: String,
    /// `fee` | `discount` | `rebate` | `amenity`.
    pub kind: String,
    pub label: String,
    /// Positive cents; the sign is derived from `kind` when applied to a lease.
    pub amount_cents: i64,
    /// Recurs monthly (vs a one-time charge).
    pub recurring: bool,
    /// `manual` | `always` | `has_pet` | `is_military` | `has_vehicle`.
    pub condition_type: String,
    /// Lease-document language for this item ({placeholder} interpolation).
    pub verbiage: Option<String>,
    pub active: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
