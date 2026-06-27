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
        ]
    }
}
