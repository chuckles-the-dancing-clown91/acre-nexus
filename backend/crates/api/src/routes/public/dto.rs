use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct ListingResp {
    pub id: Uuid,
    pub title: String,
    pub address: String,
    pub city: String,
    pub beds: i32,
    pub baths: i32,
    pub sqft: i32,
    pub rent_cents: i64,
    pub rent_label: String,
    pub status: String,
    pub available_on: String,
    pub description: String,
}

impl From<entity::listing::Model> for ListingResp {
    fn from(l: entity::listing::Model) -> Self {
        ListingResp {
            rent_label: usd(l.rent_cents),
            id: l.id,
            title: l.title,
            address: l.address,
            city: l.city,
            beds: l.beds,
            baths: l.baths,
            sqft: l.sqft,
            rent_cents: l.rent_cents,
            status: l.status,
            available_on: l.available_on,
            description: l.description,
        }
    }
}

/// Public branding so a white-label site can theme itself before login.
#[derive(Serialize, schemars::JsonSchema)]
pub struct PublicTheme {
    pub company_name: String,
    pub logo_url: Option<String>,
    pub primary_color: String,
    pub accent_color: String,
    pub default_mode: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ApplyReq {
    pub listing_id: Option<Uuid>,
    pub applicant_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub annual_income_cents: Option<i64>,
    pub credit_score: Option<i32>,
    pub move_in: Option<String>,
    /// Renter attributes that carry into the lease + drive conditional charges.
    pub has_pet: Option<bool>,
    pub pet_details: Option<String>,
    pub is_military: Option<bool>,
    /// The applicant authorizes a consumer report — credit, criminal, and
    /// eviction history (FCRA §604(b)). Required unless a recent approval is
    /// being reused.
    pub screening_consent: Option<bool>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ApplyResp {
    pub application_id: Uuid,
    pub status: String,
    /// Id of the enqueued background-screening job (Tokio scheduler).
    pub screening_job_id: Uuid,
    pub message: String,
}
