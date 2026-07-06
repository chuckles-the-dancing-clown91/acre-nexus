//! Shapes for property onboarding — the full intake payload an investor submits
//! to bring a house onto the platform in one call (property + financing).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A mortgage/loan to attach during onboarding. The lender can be an existing
/// registry entity (`lender_id`) or a name to create one on the fly.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct OnboardMortgage {
    pub lender_id: Option<Uuid>,
    /// If set (and `lender_id` is not), a lender counterparty is created.
    pub lender_name: Option<String>,
    pub kind: String,
    pub position: Option<i32>,
    pub original_amount_cents: Option<i64>,
    pub current_balance_cents: Option<i64>,
    pub interest_rate_bps: Option<i32>,
    pub term_months: Option<i32>,
    pub monthly_payment_cents: Option<i64>,
    pub escrow_monthly_cents: Option<i64>,
    pub start_date: Option<String>,
    pub maturity_date: Option<String>,
    pub loan_number: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct OnboardReq {
    // ---- Property core ----
    pub name: String,
    pub address: String,
    pub city: String,
    pub llc_id: Option<Uuid>,
    pub portfolio_id: Option<Uuid>,
    pub units: Option<i32>,
    pub occupied_units: Option<i32>,
    pub monthly_rent_cents: Option<i64>,
    pub year_built: Option<i32>,
    pub manager: Option<String>,
    pub status: Option<String>,
    // ---- Investor classification ----
    /// `single_family` | `multi_family` | `condo` | `townhome` | `commercial` | `land`.
    pub property_type: String,
    /// `rental` | `flip` | `brrrr` | `hold` | `wholesale`.
    pub strategy: String,
    pub purchase_price_cents: Option<i64>,
    pub acquired_on: Option<String>,
    /// Hero photo URL for the property profile.
    pub image_url: Option<String>,
    // ---- Financing ----
    #[serde(default)]
    pub mortgages: Vec<OnboardMortgage>,
    // ---- Team ----
    /// Staff to assign to the new property (property manager, landlord, …). Each
    /// assignment also grants that person `property:{id}`-scoped access.
    #[serde(default)]
    pub assignments: Vec<crate::routes::assignments::CreateAssignmentReq>,
    /// Whether to kick off automated enrichment after onboarding (default true).
    #[serde(default = "default_true")]
    pub enrich: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct OnboardResp {
    pub property_id: Uuid,
    pub strategy: String,
    pub workflow_stage: String,
    pub mortgages_created: usize,
    pub lenders_created: usize,
    pub assignments_created: usize,
    /// The enrichment orchestrator job, if enrichment was requested.
    pub enrich_job_id: Option<Uuid>,
}
