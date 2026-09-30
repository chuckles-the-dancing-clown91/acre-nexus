//! An expense or mileage trip, optionally tied to a work order / rehab project / property / asset and billable to the owner. Mileage stores hundredths of a mile and the rate applied in mills ($0.001).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "expense")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub incurred_on: String,
    pub category: String,
    pub vendor: Option<String>,
    pub description: String,
    pub amount_cents: i64,
    pub miles_hundredths: Option<i64>,
    pub mileage_rate_mills: Option<i64>,
    pub tax_deductible: bool,
    /// `company` | `personal` | `none`
    pub vehicle: String,
    pub reimbursable: bool,
    pub reimbursed_at: Option<DateTimeWithTimeZone>,
    pub billable_to_owner: bool,
    pub billed_bill_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub maintenance_ticket_id: Option<Uuid>,
    pub rehab_project_id: Option<Uuid>,
    pub property_id: Option<Uuid>,
    pub asset_id: Option<Uuid>,
    pub details: Json,
    pub recorded_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
