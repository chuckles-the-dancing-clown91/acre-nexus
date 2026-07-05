use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct BankAccountResp {
    pub id: Uuid,
    pub entity_id: Uuid,
    pub kind: String,
    pub institution: String,
    pub masked_number: Option<String>,
    pub status: String,
    /// `plaid` once linked for feeds.
    pub provider: Option<String>,
    pub linked: bool,
    pub last_synced_at: Option<String>,
}

impl From<entity::bank_account::Model> for BankAccountResp {
    fn from(a: entity::bank_account::Model) -> Self {
        BankAccountResp {
            id: a.id,
            entity_id: a.entity_id,
            kind: a.kind,
            institution: a.institution,
            masked_number: a.masked_number,
            status: a.status,
            provider: a.provider,
            linked: a.external_id.is_some(),
            last_synced_at: a.last_synced_at.map(|t| t.to_rfc3339()),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct BankTxnDto {
    pub id: Uuid,
    pub bank_account_id: Uuid,
    pub posted_date: String,
    pub description: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub status: String,
    pub matched_payment_id: Option<Uuid>,
}

impl From<entity::bank_txn::Model> for BankTxnDto {
    fn from(t: entity::bank_txn::Model) -> Self {
        BankTxnDto {
            amount_label: crate::dto::usd(t.amount_cents),
            id: t.id,
            bank_account_id: t.bank_account_id,
            posted_date: t.posted_date,
            description: t.description,
            amount_cents: t.amount_cents,
            status: t.status,
            matched_payment_id: t.matched_payment_id,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct LinkAccountReq {
    /// Plaid Link public token (live mode only; simulation needs nothing).
    pub public_token: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct MatchTxnReq {
    pub payment_id: Uuid,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateBankAccountReq {
    /// `operating` | `trust`.
    pub kind: String,
    pub institution: String,
    /// Full or partial account number; only the last 4 are retained, masked.
    pub account_number: Option<String>,
}
