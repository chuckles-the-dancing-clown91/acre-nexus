//! **Crime statistics** for a property's area: the nearest law-enforcement
//! agency's offense rates per 100,000 people over the last year, against the
//! state and the country, from the FBI's Crime Data Explorer (or a simulation
//! when it isn't reachable). One row per property, replaced on refresh.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "property_crime")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub property_id: Uuid,
    pub tenant_id: Uuid,
    /// The agency's originating identifier, e.g. `OR0260200`.
    pub agency_ori: Option<String>,
    pub agency_name: String,
    /// How far the agency's seat is from the property, when known.
    pub agency_km: Option<f64>,
    /// `MM-YYYY`.
    pub period_from: String,
    pub period_to: String,
    pub population: Option<i64>,
    /// `[{key, label, agency, agency_rate, state_rate, us_rate, prior_rate}]`.
    pub offenses: Json,
    /// `well_below` | `below` | `about` | `above` | `well_above` | `unknown`:
    /// the agency's violent + property rate against the state's.
    pub verdict: String,
    pub source: String,
    pub fetched_at: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
