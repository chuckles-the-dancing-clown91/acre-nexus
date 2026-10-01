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
    /// When it was listed (ISO 8601), for sitemaps and structured data.
    pub listed_at: String,
    /// What's in the home — marketing copy for the appliances on record.
    pub appliances: Vec<PublicAppliance>,
    /// The upkeep the home gets on a schedule (filters, servicing, sweeps).
    pub upkeep: Vec<PublicUpkeep>,
}

#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct PublicAppliance {
    pub kind: String,
    pub name: String,
    pub make: Option<String>,
    pub model: Option<String>,
    /// The year it went in, when known.
    pub since: Option<String>,
    pub under_warranty: bool,
}

#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct PublicUpkeep {
    pub title: String,
    /// "Every 3 months", "Yearly"…
    pub cadence: String,
}

/// Cadence in days → words a renter reads (pure).
pub fn cadence_words(days: i32) -> String {
    match days {
        d if d <= 0 => "As needed".into(),
        1 => "Daily".into(),
        7 => "Weekly".into(),
        14 => "Every 2 weeks".into(),
        28..=31 => "Monthly".into(),
        d if d % 30 == 0 && d < 360 => format!("Every {} months", d / 30),
        d if (85..=95).contains(&d) => "Every 3 months".into(),
        d if (175..=185).contains(&d) => "Every 6 months".into(),
        d if (360..=370).contains(&d) => "Yearly".into(),
        d if d % 365 == 0 => format!("Every {} years", d / 365),
        d => format!("Every {d} days"),
    }
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
            listed_at: l.created_at.to_rfc3339(),
            appliances: Vec::new(),
            upkeep: Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::cadence_words;

    #[test]
    fn cadence_reads_like_english() {
        assert_eq!(cadence_words(90), "Every 3 months");
        assert_eq!(cadence_words(30), "Monthly");
        assert_eq!(cadence_words(365), "Yearly");
        assert_eq!(cadence_words(730), "Every 2 years");
        assert_eq!(cadence_words(45), "Every 45 days");
    }
}
