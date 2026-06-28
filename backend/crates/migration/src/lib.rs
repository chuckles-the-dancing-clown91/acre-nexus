//! Database migrations for the Acre platform.
//!
//! Since the database split, schema lives in the three per-domain crates and is
//! applied to three separate databases. This crate is a thin facade that
//! re-exports each domain's migrator under a clear name:
//!
//! - [`UserMigrator`] → `acre_user`
//! - [`PropertyMigrator`] → `acre_property`
//! - [`ClientMigrator`] → `acre_client`
//!
//! Run all three with the `migration` binary (`cargo run -p migration -- up`),
//! or programmatically at server boot (see the `api` crate's `main.rs`).

pub use sea_orm_migration::prelude::MigratorTrait;

pub use acre_client::migration::Migrator as ClientMigrator;
pub use acre_property::migration::Migrator as PropertyMigrator;
pub use acre_user::migration::Migrator as UserMigrator;
