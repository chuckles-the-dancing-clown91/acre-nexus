//! Migrations for the **user** domain database (`acre_user`).
//!
//! Run by [`Migrator`] against the `acre_user` connection at boot (as the
//! schema-owner role). Covers identity/auth/RBAC/tenancy plus the two
//! cross-cutting platform tables (`audit_log`, `background_job`).

use sea_orm_migration::prelude::*;

mod m20240101_000001_init;
mod m20240101_000002_rls;
mod m20240101_000003_modules;
mod m20240101_000004_users_rbac;
mod m20240101_000005_audit;
mod m20240101_000006_audit_request;
mod m20240101_000007_storage_email;
mod m20240101_000008_job_retry;

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
            Box::new(m20240101_000007_storage_email::Migration),
            Box::new(m20240101_000008_job_retry::Migration),
        ]
    }
}

// ---- shared column helpers for the hand-split migrations in this crate ----

/// A `TIMESTAMPTZ NOT NULL DEFAULT now()` column.
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
