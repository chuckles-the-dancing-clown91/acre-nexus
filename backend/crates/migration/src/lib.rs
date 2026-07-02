//! Database migrations for the Acre platform.
//!
//! Run with the `migration` binary (`cargo run -p migration -- up`) or
//! programmatically via [`Migrator`] at server boot.

pub use sea_orm_migration::prelude::*;

mod m20240101_000001_init;
mod m20240101_000002_rls;
mod m20240101_000003_modules;
mod m20240101_000004_users_rbac;
mod m20240101_000005_audit;
mod m20240101_000006_audit_request;
mod m20240101_000007_property_data;
mod m20240101_000008_investing;
mod m20240101_000009_rentals_title;
mod m20240101_000010_tenancy_entities;
mod m20240101_000011_platform_plane;
mod m20240101_000012_domains_onboarding;
mod m20240101_000013_leasing_lifecycle;
mod m20240101_000014_lease_doc_signature;
mod m20240101_000015_rls_enforce;
mod m20240101_000016_assignments;
mod m20240101_000017_settings_app_workflow;
mod m20240101_000018_integrations;
mod m20240101_000019_notifications;
mod m20240101_000020_esign;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_init::Migration),
            Box::new(m20240101_000002_rls::Migration),
            Box::new(m20240101_000003_modules::Migration),
            Box::new(m20240101_000004_users_rbac::Migration),
            Box::new(m20240101_000005_audit::Migration),
            Box::new(m20240101_000006_audit_request::Migration),
            Box::new(m20240101_000007_property_data::Migration),
            Box::new(m20240101_000008_investing::Migration),
            Box::new(m20240101_000009_rentals_title::Migration),
            Box::new(m20240101_000010_tenancy_entities::Migration),
            Box::new(m20240101_000011_platform_plane::Migration),
            Box::new(m20240101_000012_domains_onboarding::Migration),
            Box::new(m20240101_000013_leasing_lifecycle::Migration),
            Box::new(m20240101_000014_lease_doc_signature::Migration),
            Box::new(m20240101_000015_rls_enforce::Migration),
            Box::new(m20240101_000016_assignments::Migration),
            Box::new(m20240101_000017_settings_app_workflow::Migration),
            Box::new(m20240101_000018_integrations::Migration),
            Box::new(m20240101_000019_notifications::Migration),
            Box::new(m20240101_000020_esign::Migration),
        ]
    }
}
