//! Migrations for the **property** domain database (`acre_property`).
//!
//! Run by [`Migrator`] against the `acre_property` connection at boot. Covers
//! the asset and all of its detail, listings, rentals, maintenance, title and
//! financing tables.

use sea_orm_migration::prelude::*;

mod m20240101_000001_init;
mod m20240101_000002_rls;
mod m20240101_000007_property_data;
mod m20240101_000008_investing;
mod m20240101_000009_rentals_title;
mod m20240101_000010_llc_onboarding;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_init::Migration),
            Box::new(m20240101_000002_rls::Migration),
            Box::new(m20240101_000007_property_data::Migration),
            Box::new(m20240101_000008_investing::Migration),
            Box::new(m20240101_000009_rentals_title::Migration),
            Box::new(m20240101_000010_llc_onboarding::Migration),
        ]
    }
}

// ---- shared column helpers for the hand-split migrations in this crate ----

pub(crate) fn ts(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name))
        .timestamp_with_time_zone()
        .not_null()
        .default(Expr::current_timestamp())
        .take()
}

pub(crate) fn uuid_pk() -> ColumnDef {
    ColumnDef::new(Alias::new("id"))
        .uuid()
        .not_null()
        .primary_key()
        .take()
}

pub(crate) fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

pub(crate) async fn index(
    manager: &SchemaManager<'_>,
    table: &str,
    column: &str,
) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name(format!("idx_{table}_{column}"))
                .table(Alias::new(table))
                .col(Alias::new(column))
                .to_owned(),
        )
        .await
}
