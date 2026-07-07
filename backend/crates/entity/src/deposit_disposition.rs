//! The security-deposit settlement at move-out: itemized deductions
//! ([`super::deposit_deduction`]), the refund executed through the payments
//! provider, and the generated statement PDF filed on the lease.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "deposit_disposition")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    pub property_id: Uuid,
    /// `draft` | `processing` | `closed` | `failed`.
    pub status: String,
    /// The deposit held when the disposition was drafted.
    pub deposit_cents: i64,
    /// Deposit minus deductions, fixed at finalize.
    pub refund_cents: Option<i64>,
    pub notes: Option<String>,
    pub provider: Option<String>,
    pub external_id: Option<String>,
    pub failure_reason: Option<String>,
    pub statement_document_id: Option<Uuid>,
    pub finalized_by: Option<Uuid>,
    pub finalized_at: Option<DateTimeWithTimeZone>,
    pub closed_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
