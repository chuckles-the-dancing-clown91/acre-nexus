use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct ApplicationResp {
    pub id: Uuid,
    pub listing_id: Option<Uuid>,
    pub applicant_name: String,
    pub email: String,
    pub phone: String,
    pub annual_income_label: String,
    pub credit_score: Option<i32>,
    pub status: String,
    pub move_in: String,
    pub has_pet: bool,
    pub pet_details: Option<String>,
    pub is_military: bool,
}

impl From<entity::application::Model> for ApplicationResp {
    fn from(a: entity::application::Model) -> Self {
        ApplicationResp {
            annual_income_label: usd(a.annual_income_cents),
            id: a.id,
            listing_id: a.listing_id,
            applicant_name: a.applicant_name,
            email: a.email,
            phone: a.phone,
            credit_score: a.credit_score,
            status: a.status,
            move_in: a.move_in,
            has_pet: a.has_pet,
            pet_details: a.pet_details,
            is_military: a.is_military,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateApplicationReq {
    /// `New` | `Screening` | `Approved` | `Declined`.
    pub status: String,
}

/// Convert an approved application into a (draft) lease.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct ConvertReq {
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub rent_cents: i64,
    pub deposit_cents: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}
