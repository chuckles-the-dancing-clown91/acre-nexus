//! One insurance certificate (COI) a vendor gave us, with its expiry.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "vendor_insurance")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub counterparty_id: Uuid,
    /// `general_liability` | `workers_comp` | `auto` | `umbrella` | `professional`.
    pub kind: String,
    pub carrier: String,
    pub policy_number: Option<String>,
    /// Per-occurrence limit.
    pub limit_cents: Option<i64>,
    /// ISO date the policy ends.
    pub expires_on: String,
    /// The certificate, when uploaded.
    pub document_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
