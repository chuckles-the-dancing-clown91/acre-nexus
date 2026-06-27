//! A **unit** is a rentable space within a [`super::property`] (an apartment,
//! suite, or the whole house for a single-family rental). Leases attach to units;
//! a unit's `status` reflects its current rental state.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "unit")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_number: String,
    pub beds: Option<i32>,
    pub baths: Option<f64>,
    pub sqft: Option<i32>,
    /// Asking/market rent in cents.
    pub market_rent_cents: Option<i64>,
    /// `occupied` | `vacant` | `make_ready` | `down`.
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
