//! # Acre domain entities (facade)
//!
//! The entity layer is split into three per-domain crates, one per database:
//! [`acre_user`] (`acre_user` db), [`acre_property`] (`acre_property` db) and
//! [`acre_client`] (`acre_client` db). This crate re-exports all of them under
//! the historical `entity::<module>` and `entity::prelude::*` paths so callers
//! need not care which crate a model physically lives in.
//!
//! ## Multi-tenancy
//! Shared-schema-per-database: every tenant-scoped row carries a `tenant_id`.
//! Application-layer guards (see the `api` crate) enforce isolation on every
//! query; Postgres row-level-security policies (in each domain's migrations)
//! provide defence in depth.
//!
//! ## Money
//! All monetary amounts are stored as **integer cents** (`i64`).
//!
//! ## Cross-domain references
//! Columns like `mortgage.lender_id` (→ a client counterparty) or
//! `property.tenant_id` (→ a user-domain tenant) are plain `Uuid`s enforced by
//! the application layer — there are **no** cross-database foreign keys.

// User / platform domain (acre_user database).
pub use acre_user::entity::{
    api_token, audit_log, background_job, membership, permission, profile_type, refresh_token, role,
    role_permission, sent_email, tenant, tenant_module, tenant_storage_config, theme, user,
    user_profile, user_role,
};

// Property domain (acre_property database).
pub use acre_property::entity::{
    enrichment_run, generated_document, lease, lease_payment, lien, listing, llc, llc_branding,
    llc_document, llc_template, maintenance_ticket, mortgage, ownership, property, property_detail,
    property_school, property_tax, property_utility, property_valuation, ticket_comment, unit,
    workflow_event,
};

// Client domain (acre_client database).
pub use acre_client::entity::{application, counterparty, counterparty_note};

pub mod prelude;
