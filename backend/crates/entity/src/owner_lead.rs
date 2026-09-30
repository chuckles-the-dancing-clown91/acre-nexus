//! A prospective owner in the management pipeline (new → contacted → proposal → won / lost).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "owner_lead")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub company: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub properties_count: i32,
    pub doors: i32,
    pub source: String,
    /// `new` | `contacted` | `proposal` | `won` | `lost`
    pub status: String,
    pub lost_reason: Option<String>,
    pub monthly_rent_cents: i64,
    pub fee_bps: Option<i32>,
    #[sea_orm(column_type = "Text", nullable)]
    pub notes: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub owner_id: Option<Uuid>,
    pub won_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
