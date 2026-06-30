use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct ChargeDto {
    pub id: Uuid,
    pub lease_id: Uuid,
    pub kind: String,
    pub code: Option<String>,
    pub label: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub recurring: bool,
    pub source: String,
    pub verbiage: Option<String>,
}

impl From<entity::lease_charge::Model> for ChargeDto {
    fn from(c: entity::lease_charge::Model) -> Self {
        ChargeDto {
            amount_label: usd(c.amount_cents),
            id: c.id,
            lease_id: c.lease_id,
            kind: c.kind,
            code: c.code,
            label: c.label,
            amount_cents: c.amount_cents,
            recurring: c.recurring,
            source: c.source,
            verbiage: c.verbiage,
        }
    }
}

/// A lease's charges plus the computed recurring monthly total (base rent + recurring charges).
#[derive(Serialize, schemars::JsonSchema)]
pub struct ChargesResp {
    pub charges: Vec<ChargeDto>,
    pub base_rent_cents: i64,
    pub base_rent_label: String,
    pub monthly_total_cents: i64,
    pub monthly_total_label: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddChargeReq {
    /// `fee` | `discount` | `rebate` | `amenity`.
    pub kind: String,
    pub code: Option<String>,
    pub label: String,
    /// Non-negative; the sign is derived from `kind`.
    pub amount_cents: i64,
    pub recurring: Option<bool>,
    pub verbiage: Option<String>,
}

/// Result of applying the fee schedule to a lease.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ApplyFeesResp {
    pub applied: usize,
    pub charges: Vec<ChargeDto>,
}
