//! A housing voucher on a lease: the housing authority pays `hap_cents` of each
//! month's rent (the HAP), and the resident owes the rest.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "housing_voucher")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    pub authority: String,
    pub contract_number: Option<String>,
    pub hap_cents: i64,
    pub starts_on: Date,
    pub ends_on: Option<Date>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
