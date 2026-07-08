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
pub mod asset;
pub mod assignment;
pub mod audit_log;
pub mod background_job;
pub mod bank_account;
pub mod bank_txn;
pub mod counterparty;
pub mod counterparty_note;
pub mod deal;
pub mod deal_event;
pub mod deposit_deduction;
pub mod deposit_disposition;
pub mod document;
pub mod domain;
pub mod enrichment_run;
pub mod entity_ownership;
pub mod esign_envelope;
pub mod esign_event;
pub mod esign_signer;
pub mod fee_schedule;
pub mod financial_snapshot;
pub mod impersonation_session;
pub mod inbound_email;
pub mod inspection;
pub mod inspection_item;
pub mod inventory_item;
pub mod lead;
pub mod lease;
pub mod lease_charge;
pub mod lease_document;
pub mod lease_payment;
pub mod ledger_account;
pub mod ledger_entry;
pub mod ledger_txn;
pub mod lien;
pub mod listing;
pub mod llc;
pub mod maintenance_plan;
pub mod maintenance_ticket;
pub mod membership;
pub mod message;
pub mod message_thread;
pub mod mortgage;
pub mod notification;
pub mod notification_provider;
pub mod onboarding_workflow;
pub mod owner;
pub mod owner_payout;
pub mod ownership;
pub mod payment_method;
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
pub mod push_subscription;
pub mod refresh_token;
pub mod rehab_change_order;
pub mod rehab_draw;
pub mod rehab_lien_waiver;
pub mod rehab_line;
pub mod rehab_project;
pub mod reminder;
pub mod role;
pub mod role_permission;
pub mod screening_report;
pub mod secret;
pub mod setting;
pub mod tenant;
pub mod tenant_module;
pub mod theme;
pub mod ticket_comment;
pub mod ticket_line;
pub mod ticket_quote;
pub mod unit;
pub mod user;
pub mod user_profile;
pub mod user_role;
pub mod vehicle;
pub mod vendor_bill;
pub mod webhook_delivery;
pub mod webhook_subscription;
pub mod workflow_event;
