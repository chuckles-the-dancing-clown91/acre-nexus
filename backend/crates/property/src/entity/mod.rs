//! SeaORM models for the **property** domain (`acre_property` database).
//!
//! Cross-domain references (e.g. `mortgage.lender_id` → a client counterparty,
//! `property.tenant_id` → a user-domain tenant) are plain `Uuid` columns
//! enforced by the application layer, never DB foreign keys.

pub mod enrichment_run;
pub mod lease;
pub mod lease_payment;
pub mod lien;
pub mod listing;
pub mod llc;
pub mod maintenance_ticket;
pub mod mortgage;
pub mod ownership;
pub mod property;
pub mod property_detail;
pub mod property_school;
pub mod property_tax;
pub mod property_utility;
pub mod property_valuation;
pub mod ticket_comment;
pub mod unit;
pub mod workflow_event;
