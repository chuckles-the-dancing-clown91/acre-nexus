use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct FeeDto {
    pub id: Uuid,
    pub code: String,
    pub kind: String,
    pub label: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub recurring: bool,
    pub condition_type: String,
    pub verbiage: Option<String>,
    pub active: bool,
}

impl From<entity::fee_schedule::Model> for FeeDto {
    fn from(f: entity::fee_schedule::Model) -> Self {
        FeeDto {
            amount_label: usd(f.amount_cents),
            id: f.id,
            code: f.code,
            kind: f.kind,
            label: f.label,
            amount_cents: f.amount_cents,
            recurring: f.recurring,
            condition_type: f.condition_type,
            verbiage: f.verbiage,
            active: f.active,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateFeeReq {
    /// Stable per-tenant code, e.g. `pet_fee`.
    pub code: String,
    /// `fee` | `discount` | `rebate` | `amenity`.
    pub kind: String,
    pub label: String,
    pub amount_cents: i64,
    pub recurring: Option<bool>,
    /// `manual` | `always` | `has_pet` | `is_military` | `has_vehicle`.
    pub condition_type: Option<String>,
    pub verbiage: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateFeeReq {
    pub label: Option<String>,
    pub amount_cents: Option<i64>,
    pub recurring: Option<bool>,
    pub condition_type: Option<String>,
    pub verbiage: Option<String>,
    pub active: Option<bool>,
}
