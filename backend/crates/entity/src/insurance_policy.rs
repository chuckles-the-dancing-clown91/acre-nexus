//! An insurance policy on a property: who carries it, what it covers, and
//! when it renews.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "insurance_policy")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// property | liability | flood | earthquake | umbrella | builders_risk | rent_loss | other
    pub kind: String,
    pub carrier: String,
    pub policy_number: Option<String>,
    /// active | cancelled | expired
    pub status: String,
    pub effective_on: Option<String>,
    pub expires_on: Option<String>,
    pub premium_cents: Option<i64>,
    pub coverage_cents: Option<i64>,
    pub deductible_cents: Option<i64>,
    pub agent_name: Option<String>,
    pub agent_phone: Option<String>,
    pub agent_email: Option<String>,
    pub document_ids: Json,
    pub notes: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
