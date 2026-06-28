//! A **school** assigned to or near a property, sourced by the enrichment engine.
//! Many rows per property (typically one per level).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "property_school")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub name: String,
    /// `elementary` | `middle` | `high`.
    pub level: String,
    pub district: Option<String>,
    /// Great-Schools-style rating 1–10.
    pub rating: Option<i32>,
    pub distance_mi: Option<f64>,
    pub grades: Option<String>,
    pub source: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
