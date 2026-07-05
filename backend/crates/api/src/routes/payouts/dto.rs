//! Request/response shapes for owner payouts.

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct PayoutDto {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub entity_name: Option<String>,
    pub period_start: String,
    pub period_end: String,
    pub rent_collected_cents: i64,
    pub rent_collected_label: String,
    pub expenses_cents: i64,
    pub expenses_label: String,
    pub mgmt_fee_cents: i64,
    pub mgmt_fee_label: String,
    pub net_cents: i64,
    pub net_label: String,
    pub status: String,
    pub statement_document_id: Option<Uuid>,
    pub ledger_txn_id: Option<Uuid>,
    pub failure_reason: Option<String>,
    pub created_at: String,
}

impl PayoutDto {
    pub fn from_model(p: entity::owner_payout::Model, entity_name: Option<String>) -> Self {
        PayoutDto {
            id: p.id,
            entity_id: p.entity_id,
            entity_name,
            period_start: p.period_start,
            period_end: p.period_end,
            rent_collected_label: usd(p.rent_collected_cents),
            rent_collected_cents: p.rent_collected_cents,
            expenses_label: usd(p.expenses_cents),
            expenses_cents: p.expenses_cents,
            mgmt_fee_label: usd(p.mgmt_fee_cents),
            mgmt_fee_cents: p.mgmt_fee_cents,
            net_label: usd(p.net_cents),
            net_cents: p.net_cents,
            status: p.status,
            statement_document_id: p.statement_document_id,
            ledger_txn_id: p.ledger_txn_id,
            failure_reason: p.failure_reason,
            created_at: p.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ComputePayoutReq {
    pub entity_id: Uuid,
    /// `YYYY-MM-DD` inclusive.
    pub period_start: String,
    /// `YYYY-MM-DD` inclusive.
    pub period_end: String,
}
