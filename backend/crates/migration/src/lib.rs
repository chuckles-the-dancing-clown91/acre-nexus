//! Database migrations for the Vantedge platform.
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
mod m20240101_000021_application_pipeline;
mod m20240101_000022_renter_profile;
mod m20240101_000023_pipeline_indexes;
mod m20240101_000024_accounting;
mod m20240101_000025_screening;
mod m20240101_000026_property_profile;
mod m20240101_000027_payables;
mod m20240101_000028_calendar;
mod m20240101_000029_email;
mod m20240101_000030_webhooks;
mod m20240101_000031_resident_portal;
mod m20240101_000032_helpdesk;
mod m20240101_000033_maintenance_full;
mod m20240101_000034_maintenance_ops;
mod m20240101_000035_deals;
mod m20240101_000036_rehab;
mod m20240101_000037_platform_billing;
mod m20240101_000038_rls_empty_guc;
mod m20240101_000039_syndication;
mod m20240101_000040_hoa;
mod m20240101_000041_leasing_crm_renewals;
mod m20240101_000042_federated_auth;
mod m20240101_000043_password_tokens;
mod m20240101_000044_sms_threads;
mod m20240101_000045_workforce;
mod m20240101_000046_crm;
mod m20240101_000047_property_geo;
mod m20240101_000048_parts;
mod m20240101_000049_partner_link;
mod m20240101_000050_audit_trail;
mod m20240101_000051_processes;
mod m20240101_000052_issue_catalog;
mod m20240101_000053_business_profile;
mod m20240101_000054_site_maps;
mod m20240101_000055_tour_requests;
mod m20240101_000056_sso;
mod m20240101_000057_embed;
mod m20240101_000058_seo;
mod m20240101_000059_notice_log;
mod m20240101_000060_vendor_compliance;
mod m20240101_000061_text_tools;
mod m20240101_000062_listing_photos;
mod m20240101_000063_service_desk;
mod m20240101_000064_imports_and_syndication;
mod m20240101_000065_maintenance_actions;
mod m20240101_000066_property_profile;
mod m20240101_000067_desk_queues;
mod m20240101_000068_property_story;
mod m20240101_000069_appointments;
mod m20240101_000070_vendor_links;
mod m20240101_000071_property_crime;
mod m20240101_000072_owner_approvals;

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
            Box::new(m20240101_000021_application_pipeline::Migration),
            Box::new(m20240101_000022_renter_profile::Migration),
            Box::new(m20240101_000023_pipeline_indexes::Migration),
            Box::new(m20240101_000024_accounting::Migration),
            Box::new(m20240101_000025_screening::Migration),
            Box::new(m20240101_000026_property_profile::Migration),
            Box::new(m20240101_000027_payables::Migration),
            Box::new(m20240101_000028_calendar::Migration),
            Box::new(m20240101_000029_email::Migration),
            Box::new(m20240101_000030_webhooks::Migration),
            Box::new(m20240101_000031_resident_portal::Migration),
            Box::new(m20240101_000032_helpdesk::Migration),
            Box::new(m20240101_000033_maintenance_full::Migration),
            Box::new(m20240101_000034_maintenance_ops::Migration),
            Box::new(m20240101_000035_deals::Migration),
            Box::new(m20240101_000036_rehab::Migration),
            Box::new(m20240101_000037_platform_billing::Migration),
            Box::new(m20240101_000038_rls_empty_guc::Migration),
            Box::new(m20240101_000039_syndication::Migration),
            Box::new(m20240101_000040_hoa::Migration),
            Box::new(m20240101_000041_leasing_crm_renewals::Migration),
            Box::new(m20240101_000042_federated_auth::Migration),
            Box::new(m20240101_000043_password_tokens::Migration),
            Box::new(m20240101_000044_sms_threads::Migration),
            Box::new(m20240101_000045_workforce::Migration),
            Box::new(m20240101_000046_crm::Migration),
            Box::new(m20240101_000047_property_geo::Migration),
            Box::new(m20240101_000048_parts::Migration),
            Box::new(m20240101_000049_partner_link::Migration),
            Box::new(m20240101_000050_audit_trail::Migration),
            Box::new(m20240101_000051_processes::Migration),
            Box::new(m20240101_000052_issue_catalog::Migration),
            Box::new(m20240101_000053_business_profile::Migration),
            Box::new(m20240101_000054_site_maps::Migration),
            Box::new(m20240101_000055_tour_requests::Migration),
            Box::new(m20240101_000056_sso::Migration),
            Box::new(m20240101_000057_embed::Migration),
            Box::new(m20240101_000058_seo::Migration),
            Box::new(m20240101_000059_notice_log::Migration),
            Box::new(m20240101_000060_vendor_compliance::Migration),
            Box::new(m20240101_000061_text_tools::Migration),
            Box::new(m20240101_000062_listing_photos::Migration),
            Box::new(m20240101_000063_service_desk::Migration),
            Box::new(m20240101_000064_imports_and_syndication::Migration),
            Box::new(m20240101_000065_maintenance_actions::Migration),
            Box::new(m20240101_000066_property_profile::Migration),
            Box::new(m20240101_000067_desk_queues::Migration),
            Box::new(m20240101_000068_property_story::Migration),
            Box::new(m20240101_000069_appointments::Migration),
            Box::new(m20240101_000070_vendor_links::Migration),
            Box::new(m20240101_000071_property_crime::Migration),
            Box::new(m20240101_000072_owner_approvals::Migration),
        ]
    }
}
