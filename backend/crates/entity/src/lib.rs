//! # Acre domain entities
//!
//! SeaORM models for the Acre multi-tenant property-management platform.
//!
//! ## Multi-tenancy
//! The platform uses a **shared-schema** model: a single Postgres database where
//! every tenant-scoped row carries a `tenant_id`. Application-layer guards
//! (see the `api` crate) enforce isolation on every query; Postgres
//! row-level-security policies (in the migration crate) provide defence in depth.
//!
//! Platform-level concepts (the `tenant` table itself, platform-staff `user`s)
//! are not tenant-scoped — they belong to "Acre HQ".
//!
//! ## Money
//! All monetary amounts are stored as **integer cents** (`i64`) to avoid
//! floating-point rounding. Format at the edges only.

pub mod prelude;

pub mod api_token;
pub mod application;
pub mod audit_log;
pub mod background_job;
pub mod enrichment_run;
pub mod listing;
pub mod llc;
pub mod membership;
pub mod permission;
pub mod profile_type;
pub mod property;
pub mod property_detail;
pub mod property_school;
pub mod property_tax;
pub mod property_utility;
pub mod property_valuation;
pub mod refresh_token;
pub mod role;
pub mod role_permission;
pub mod tenant;
pub mod tenant_module;
pub mod theme;
pub mod user;
pub mod user_profile;
pub mod user_role;
