//! # Acre **property** domain
//!
//! The real-estate asset and everything about it — properties and their
//! enrichment detail, listings, rentals (units/leases/payments), maintenance,
//! title (ownership/liens), financing (mortgages/LLCs) and the investment
//! workflow. These tables are hosted in the `acre_property` database.
//!
//! Money is stored as integer cents (`i64`); see individual models.

pub mod entity;
pub mod migration;
