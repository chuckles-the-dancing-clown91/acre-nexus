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
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateApplicationReq {
    /// `New` | `Screening` | `Approved` | `Declined`.
    pub status: String,
}
