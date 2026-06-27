//! An automated **valuation** (AVM) snapshot for a property — an estimated
//! market value and estimated market rent with a confidence band, à la a
//! "Zestimate". Many rows per property form a value-over-time history.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "property_valuation")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// Effective date of the estimate (`YYYY-MM-DD`).
    pub as_of: String,
    pub estimated_value_cents: Option<i64>,
    pub value_low_cents: Option<i64>,
    pub value_high_cents: Option<i64>,
    /// Estimated market rent, in cents/month.
    pub estimated_rent_cents: Option<i64>,
    /// Confidence 0–100.
    pub confidence: Option<i32>,
    pub source: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
