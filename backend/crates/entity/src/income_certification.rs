//! A household's income certified against an income limit (a percent of area
//! median income for its size), for a lease in a foundation entity.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "income_certification")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    pub effective_on: Date,
    /// Recertify by this day.
    pub expires_on: Date,
    pub household_size: i32,
    pub annual_income_cents: i64,
    /// Area median income for the household's size.
    pub ami_cents: i64,
    /// The limit as a percent of AMI (50, 60, 80 …).
    pub limit_pct: i32,
    pub qualified: bool,
    pub notes: Option<String>,
    pub certified_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
