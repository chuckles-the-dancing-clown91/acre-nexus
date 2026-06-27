//! A **property tax / assessment** record for one tax year, sourced from county
//! assessor data by the enrichment engine. Many rows per property (one per year).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "property_tax")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub tax_year: i32,
    /// Total assessed value, in cents.
    pub assessed_value_cents: Option<i64>,
    pub land_value_cents: Option<i64>,
    pub improvement_value_cents: Option<i64>,
    /// Annual tax billed, in cents.
    pub tax_amount_cents: Option<i64>,
    /// Effective tax rate in basis points (1% = 100 bps).
    pub tax_rate_bps: Option<i32>,
    /// Where the figure came from (provider key).
    pub source: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
