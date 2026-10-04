//! A reading taken from a meter.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "meter_reading")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub meter_id: Uuid,
    pub read_on: Date,
    pub reading: f64,
    /// `routine` | `move_in` | `move_out` | `other`.
    pub reason: String,
    pub lease_id: Option<Uuid>,
    pub note: Option<String>,
    pub read_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
