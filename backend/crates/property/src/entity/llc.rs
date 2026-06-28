//! An **LLC** is a legal holding entity owned by a tenant. Properties are grouped
//! under LLCs (e.g. "Maple Holdings LLC" owns The Maple Court & Birchwood Lofts).
//!
//! Beyond the bare registration facts (`name` / `ein` / `state`), an LLC carries
//! an **onboarding profile**: contact + filing details and a `status` that walks
//! `draft → pending_docs → active`. Its uploaded documents, branding/signature,
//! and document templates live in the sibling `llc_document`, `llc_branding`, and
//! `llc_template` tables.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "llc")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub ein: String,
    /// Two-letter state of registration.
    pub state: String,
    // ---- onboarding profile ----
    /// Legal entity type: `LLC` | `C-Corp` | `S-Corp` | `LP` | `sole_prop` | `trust`.
    pub entity_type: String,
    /// Date the entity was formed (YYYY-MM-DD).
    pub formation_date: Option<String>,
    pub registered_agent: Option<String>,
    pub principal_address: Option<String>,
    pub mailing_address: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub website: Option<String>,
    /// Onboarding lifecycle: `draft` | `pending_docs` | `active` | `suspended`.
    pub status: String,
    /// When onboarding was marked complete (status → `active`).
    pub onboarded_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
