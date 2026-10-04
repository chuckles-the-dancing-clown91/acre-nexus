//! A season at a campground: dates (month-day, may wrap the new year), a price
//! adjustment and a minimum stay.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "campground_season")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub map_id: Uuid,
    pub name: String,
    /// `MM-DD`
    pub start_md: String,
    /// `MM-DD`, inclusive
    pub end_md: String,
    pub adjust_pct: i32,
    pub min_nights: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
