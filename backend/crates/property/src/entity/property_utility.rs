//! A **utility** servicing a property (the provider and a typical monthly cost),
//! sourced by the enrichment engine. Many rows per property (one per utility type).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "property_utility")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// `electric` | `gas` | `water` | `sewer` | `trash` | `internet`.
    pub utility_type: String,
    pub provider: String,
    pub est_monthly_cost_cents: Option<i64>,
    pub phone: Option<String>,
    pub source: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
