//! The clock: time against a work order, rehab project or property (or travel / shop / office). An entry with no `ended_at` is someone clocked in. Pay and bill rates are frozen when it closes.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "time_entry")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    /// `work_order` | `project` | `property` | `travel` | `shop` | `admin` | `other`
    pub kind: String,
    pub maintenance_ticket_id: Option<Uuid>,
    pub rehab_project_id: Option<Uuid>,
    pub property_id: Option<Uuid>,
    pub started_at: DateTimeWithTimeZone,
    pub ended_at: Option<DateTimeWithTimeZone>,
    pub break_minutes: i32,
    pub notes: Option<String>,
    pub pay_rate_cents: Option<i64>,
    pub bill_rate_cents: Option<i64>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTimeWithTimeZone>,
    pub missed_punch: bool,
    pub missed_punch_reason: Option<String>,
    pub claimed_end: Option<DateTimeWithTimeZone>,
    pub punch_note: Option<String>,
    pub resolved_by: Option<Uuid>,
    pub resolved_at: Option<DateTimeWithTimeZone>,
    pub in_lat: Option<f64>,
    pub in_lng: Option<f64>,
    pub in_distance_m: Option<i32>,
    pub out_lat: Option<f64>,
    pub out_lng: Option<f64>,
    pub out_distance_m: Option<i32>,
    pub billed_bill_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
