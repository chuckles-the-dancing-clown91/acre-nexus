//! Request/response shapes for CRM leads.

use crate::routes::applications::dto::ApplicationResp;
use crate::routes::reminders::dto::ReminderDto;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lead statuses, in pipeline order.
pub const STATUSES: &[&str] = &["new", "contacted", "toured", "applied", "closed"];

/// Lead sources a lead can be entered under.
pub const SOURCES: &[&str] = &["inbound_email", "manual", "website", "referral", "walk_in"];

#[derive(Serialize, schemars::JsonSchema)]
pub struct LeadDto {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub source: String,
    pub status: String,
    pub notes: Option<String>,
    pub last_message: Option<String>,
    /// The application this lead was converted into, if any.
    pub application_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::lead::Model> for LeadDto {
    fn from(l: entity::lead::Model) -> Self {
        LeadDto {
            id: l.id,
            name: l.name,
            email: l.email,
            phone: l.phone,
            source: l.source,
            status: l.status,
            notes: l.notes,
            last_message: l.last_message,
            application_id: l.application_id,
            created_at: l.created_at.to_rfc3339(),
            updated_at: l.updated_at.to_rfc3339(),
        }
    }
}

/// The leads list plus the tenant's monitored inbox address (mail sent there
/// creates/updates leads).
#[derive(Serialize, schemars::JsonSchema)]
pub struct LeadsResp {
    pub inbox_address: Option<String>,
    pub leads: Vec<LeadDto>,
}

/// Manually enter a prospect into the CRM.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateLeadReq {
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    /// Defaults to `manual`. One of `manual` | `website` | `referral` | `walk_in`.
    pub source: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateLeadReq {
    pub name: Option<String>,
    pub phone: Option<String>,
    /// `new` | `contacted` | `toured` | `applied` | `closed`.
    pub status: Option<String>,
    pub notes: Option<String>,
}

/// Schedule a tour for a lead — creates a `tour` reminder on the calendar and
/// advances the lead into the pipeline.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct ScheduleTourReq {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub notes: Option<String>,
    /// Days before the tour to notify. Defaults to the workspace's calendar
    /// default lead days.
    pub lead_days: Option<Vec<i64>>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ScheduleTourResp {
    pub lead: LeadDto,
    pub reminder: ReminderDto,
}

/// Convert a lead into a rental application without leaving the platform. The
/// lead's contact details seed the application; the lead is marked `applied`
/// and linked to the new application.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct ConvertLeadReq {
    pub listing_id: Option<Uuid>,
    pub annual_income_cents: Option<i64>,
    pub credit_score: Option<i32>,
    pub move_in: Option<String>,
    pub has_pet: Option<bool>,
    pub pet_details: Option<String>,
    pub is_military: Option<bool>,
    /// Staff attest the applicant authorized a consumer report (defaults true —
    /// back-office conversion implies the paperwork happened offline).
    pub screening_consent: Option<bool>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct ConvertLeadResp {
    pub lead: LeadDto,
    pub application: ApplicationResp,
}
