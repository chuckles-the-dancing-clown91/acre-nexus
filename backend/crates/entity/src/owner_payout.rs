//! An **owner payout** (draw) closes the loop from "rent collected" to "owner
//! got paid": computed from one entity's ledger for a period (rent collected −
//! expenses − management fee), executed as an ACH transfer via the payments
//! provider, posted to the ledger, and documented with a generated statement.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "owner_payout")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// FK to `llc.id` — the entity whose owners are being paid.
    pub entity_id: Uuid,
    /// `YYYY-MM-DD` inclusive.
    pub period_start: String,
    /// `YYYY-MM-DD` inclusive.
    pub period_end: String,
    pub rent_collected_cents: i64,
    pub expenses_cents: i64,
    pub mgmt_fee_cents: i64,
    /// rent collected − expenses − management fee (never negative to execute).
    pub net_cents: i64,
    /// `draft` | `processing` | `paid` | `failed`.
    pub status: String,
    pub provider: Option<String>,
    /// Provider transfer id (`po_…` / `sim_po_…`).
    pub external_id: Option<String>,
    /// The generated owner statement in the document service.
    pub statement_document_id: Option<Uuid>,
    /// The ledger posting recorded when the payout settled.
    pub ledger_txn_id: Option<Uuid>,
    pub failure_reason: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
