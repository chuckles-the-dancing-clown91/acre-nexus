//! Request/response shapes for lease-renewal endpoints.

use crate::dto::usd;
use crate::leasedoc::rent_change_label;
use crate::routes::esign::dto::{EnvelopeDto, SignerLink, SignerReq};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct RenewalDto {
    pub id: Uuid,
    pub lease_id: Uuid,
    /// `proposed` | `sent` | `signed` | `activated` | `declined` | `cancelled`.
    pub status: String,
    pub current_rent_cents: i64,
    pub current_rent_label: String,
    pub new_rent_cents: i64,
    pub new_rent_label: String,
    /// Human summary of the rent change, e.g. `"+$150 / month (+8.3%)"`.
    pub rent_change_label: String,
    pub new_start_date: String,
    pub new_end_date: Option<String>,
    pub term_months: Option<i32>,
    pub notes: Option<String>,
    /// The generated addendum document and the envelope it was sent in.
    pub lease_document_id: Option<Uuid>,
    pub envelope_id: Option<Uuid>,
    /// The signing envelope with its signers + audit trail, when one has been
    /// sent — surfaced on the list so the console can track signing progress.
    pub envelope: Option<EnvelopeDto>,
    pub activated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::lease_renewal::Model> for RenewalDto {
    fn from(r: entity::lease_renewal::Model) -> Self {
        RenewalDto::build(r, None)
    }
}

impl RenewalDto {
    /// Build a renewal DTO, optionally embedding its signing envelope.
    pub fn build(r: entity::lease_renewal::Model, envelope: Option<EnvelopeDto>) -> Self {
        RenewalDto {
            current_rent_label: usd(r.current_rent_cents),
            new_rent_label: usd(r.new_rent_cents),
            rent_change_label: rent_change_label(r.current_rent_cents, r.new_rent_cents),
            id: r.id,
            lease_id: r.lease_id,
            status: r.status,
            current_rent_cents: r.current_rent_cents,
            new_rent_cents: r.new_rent_cents,
            new_start_date: r.new_start_date,
            new_end_date: r.new_end_date,
            term_months: r.term_months,
            notes: r.notes,
            lease_document_id: r.lease_document_id,
            envelope_id: r.envelope_id,
            envelope,
            activated_at: r.activated_at.map(|x| x.to_rfc3339()),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        }
    }
}

/// Propose a renewal. Provide the new rent, plus either a `term_months` (the
/// end date is computed from the effective start) or an explicit `new_end_date`
/// (omit both for a month-to-month renewal). `new_start_date` defaults to the
/// day after the current lease end.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct ProposeRenewalReq {
    pub new_rent_cents: i64,
    pub term_months: Option<i32>,
    pub new_start_date: Option<String>,
    pub new_end_date: Option<String>,
    pub notes: Option<String>,
}

/// The proposed renewal plus the addendum document generated for it.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ProposeRenewalResp {
    pub renewal: RenewalDto,
    /// The rendered addendum body (for preview), and its document id.
    pub document_id: Uuid,
    pub document_body: String,
}

/// Send the addendum for signature. Signers default to the lease's resident +
/// the sending user (as landlord), exactly like the initial lease envelope.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct SendRenewalReq {
    pub message: Option<String>,
    pub signers: Option<Vec<SignerReq>>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SendRenewalResp {
    pub renewal: RenewalDto,
    pub envelope: EnvelopeDto,
    /// Signing links, for copy/paste — also emailed (and texted) to signers.
    pub sign_links: Vec<SignerLink>,
}
