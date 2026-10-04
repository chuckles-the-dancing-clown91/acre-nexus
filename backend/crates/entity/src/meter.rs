//! A utility meter on a property (common) or on one of its units, and who
//! pays for it.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "meter")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// None for a house meter that serves the whole property.
    pub unit_id: Option<Uuid>,
    /// `electric` | `gas` | `water` | `sewer` | `trash` | `internet` | `other`.
    pub kind: String,
    pub label: String,
    pub meter_number: Option<String>,
    pub location: Option<String>,
    pub provider: Option<String>,
    /// kWh, therms, gallons, ccf …
    pub unit_of_measure: Option<String>,
    /// `tenant` | `landlord` | `shared`.
    pub paid_by: String,
    pub billing_note: Option<String>,
    /// `active` | `retired`.
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
