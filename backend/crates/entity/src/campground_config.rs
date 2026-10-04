//! Booking rules for one campground (site map).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "campground_config")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub map_id: Uuid,
    pub tenant_id: Uuid,
    pub booking_open: bool,
    pub deposit_pct: i32,
    pub check_in_time: String,
    pub check_out_time: String,
    pub max_nights: i32,
    /// `[{ key, label, price_cents, per: "stay" | "night" }]`
    pub addons: Json,
    pub policies: Option<String>,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
