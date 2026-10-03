//! A permit pulled on a property: the work it covers, where it stands with
//! the building department, and the dates that matter.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "property_permit")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub permit_number: Option<String>,
    /// building | electrical | plumbing | mechanical | roofing | demolition | fence | solar | other
    pub kind: String,
    pub description: String,
    /// applied | issued | inspection | finaled | expired | void
    pub status: String,
    pub jurisdiction: Option<String>,
    pub applied_on: Option<String>,
    pub issued_on: Option<String>,
    pub expires_on: Option<String>,
    /// Next inspection.
    pub inspection_on: Option<String>,
    pub finaled_on: Option<String>,
    pub contractor_entity_id: Option<Uuid>,
    pub contractor_name: Option<String>,
    pub valuation_cents: Option<i64>,
    pub fee_cents: Option<i64>,
    pub ticket_id: Option<Uuid>,
    pub document_ids: Json,
    pub notes: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
