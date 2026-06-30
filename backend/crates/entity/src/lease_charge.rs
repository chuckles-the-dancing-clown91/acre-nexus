//! A **lease charge** is one resolved line item on a lease — base-rent add-ons,
//! pet fees, military discounts, garage/amenity charges, prorations. Discounts and
//! rebates are stored as **negative** `amount_cents`. A charge may originate from a
//! [`super::fee_schedule`] (carrying its `code` + `verbiage`) or be added manually.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "lease_charge")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    /// `fee` | `discount` | `rebate` | `amenity`.
    pub kind: String,
    /// The originating `fee_schedule.code`, if any.
    pub code: Option<String>,
    pub label: String,
    /// Positive for charges, negative for discounts/rebates.
    pub amount_cents: i64,
    /// Recurs monthly (vs one-time).
    pub recurring: bool,
    /// `manual` | `auto` (matched a condition) | `application`.
    pub source: String,
    /// Snapshot of the lease-document language at the time it was applied.
    pub verbiage: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
