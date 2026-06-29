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
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateBankAccountReq {
    /// `operating` | `trust`.
    pub kind: String,
    pub institution: String,
    /// Full or partial account number; only the last 4 are retained, masked.
    pub account_number: Option<String>,
}
