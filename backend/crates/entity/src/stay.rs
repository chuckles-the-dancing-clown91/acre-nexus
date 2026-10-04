//! A stay on a campsite.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "stay")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub map_id: Uuid,
    /// The site's feature id on the map.
    pub site_id: Uuid,
    /// The site's name when booked.
    pub site_name: String,
    pub guest_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub check_in: Date,
    pub check_out: Date,
    pub guests: i32,
    pub vehicle: Option<String>,
    pub rig_length_ft: Option<i32>,
    /// `[{ key, label, qty, cents }]`
    pub addons: Json,
    pub total_cents: i64,
    pub deposit_cents: i64,
    pub paid_cents: i64,
    /// `held` | `confirmed` | `checked_in` | `checked_out` | `cancelled`
    pub status: String,
    /// `staff` | `public`
    pub source: String,
    pub note: Option<String>,
    pub token_hash: Option<String>,
    pub checked_in_at: Option<DateTimeWithTimeZone>,
    pub checked_out_at: Option<DateTimeWithTimeZone>,
    pub cleaned_at: Option<DateTimeWithTimeZone>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
