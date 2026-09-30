//! Employee profile — HR and pay details for a staff member in a workspace (1:1 with a user). `bill_rate_cents` is what an hour of their work is charged to owners.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "employee_profile")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub title: Option<String>,
    /// `full_time` | `part_time` | `seasonal` | `contractor` (1099)
    pub employment_type: String,
    pub pay_rate_cents: i64,
    pub bill_rate_cents: i64,
    pub hire_date: Option<String>,
    pub end_date: Option<String>,
    pub weekly_hours_target: i32,
    /// `company` | `personal`
    pub default_vehicle: String,
    pub mileage_reimbursed: bool,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub calendar_color: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
