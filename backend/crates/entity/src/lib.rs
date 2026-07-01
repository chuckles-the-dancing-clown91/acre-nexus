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
pub mod application_event;
pub mod assignment;
pub mod audit_log;
pub mod background_job;
pub mod bank_account;
pub mod counterparty;
pub mod counterparty_note;
pub mod domain;
pub mod enrichment_run;
pub mod entity_ownership;
pub mod fee_schedule;
pub mod impersonation_session;
pub mod lease;
pub mod lease_charge;
pub mod lease_document;
pub mod lease_payment;
pub mod lien;
pub mod listing;
pub mod llc;
pub mod maintenance_ticket;
pub mod membership;
pub mod mortgage;
pub mod onboarding_workflow;
pub mod owner;
pub mod ownership;
pub mod permission;
pub mod platform_staff;
pub mod portfolio;
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
pub mod setting;
pub mod tenant;
pub mod tenant_module;
pub mod theme;
pub mod ticket_comment;
pub mod unit;
pub mod user;
pub mod user_profile;
pub mod user_role;
pub mod vehicle;
pub mod workflow_event;
