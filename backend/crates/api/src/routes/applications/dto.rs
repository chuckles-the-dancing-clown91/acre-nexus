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
    /// Intake door: `public` | `portal` | `back_office`.
    pub source: String,
    /// Background-check outcome once screening finishes: `cleared` | `failed`.
    pub screening_status: Option<String>,
    pub screened_at: Option<String>,
    /// When the applicant authorized the consumer report (FCRA §604(b)).
    pub screening_consent_at: Option<String>,
    /// When the FCRA §615(a) adverse-action notice was sent, if it was.
    pub adverse_action_at: Option<String>,
    pub adverse_action_document_id: Option<Uuid>,
    pub created_at: String,
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
            source: a.source,
            screening_status: a.screening_status,
            screened_at: a.screened_at.map(|x| x.to_rfc3339()),
            screening_consent_at: a.screening_consent_at.map(|x| x.to_rfc3339()),
            adverse_action_at: a.adverse_action_at.map(|x| x.to_rfc3339()),
            adverse_action_document_id: a.adverse_action_document_id,
            created_at: a.created_at.to_rfc3339(),
        }
    }
}

/// Back-office intake: staff enter an application on an applicant's behalf.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateApplicationReq {
    pub listing_id: Option<Uuid>,
    pub applicant_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub annual_income_cents: Option<i64>,
    pub credit_score: Option<i32>,
    pub move_in: Option<String>,
    pub has_pet: Option<bool>,
    pub pet_details: Option<String>,
    pub is_military: Option<bool>,
    /// Staff attest the applicant authorized a consumer report (e.g. a signed
    /// paper form). Defaults to true — back-office intake implies the
    /// paperwork happened outside the system.
    pub screening_consent: Option<bool>,
}

/// Renter-portal application: identity comes from the signed-in account, so
/// everything here is optional detail.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct PortalApplyReq {
    pub listing_id: Option<Uuid>,
    /// Defaults to the account's display name.
    pub applicant_name: Option<String>,
    /// Defaults to the profile's phone.
    pub phone: Option<String>,
    pub annual_income_cents: Option<i64>,
    pub credit_score: Option<i32>,
    pub move_in: Option<String>,
    pub has_pet: Option<bool>,
    pub pet_details: Option<String>,
    pub is_military: Option<bool>,
    /// The applicant authorizes a consumer report — credit, criminal, and
    /// eviction history (FCRA §604(b)). Required unless a recent approval is
    /// being reused.
    pub screening_consent: Option<bool>,
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
    /// Auto-generate the draft lease agreement (default true).
    pub generate_document: Option<bool>,
}
