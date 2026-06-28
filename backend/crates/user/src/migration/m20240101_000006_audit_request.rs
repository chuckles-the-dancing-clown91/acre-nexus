//! Extends `audit_log` so it can record **every API request**, not just the
//! handful of sensitive domain events the original table captured.
//!
//! The new columns describe the HTTP request that produced the entry (method,
//! path, status, latency), the kind of principal behind it, and a per-request
//! correlation id. All are nullable so the existing domain-event writer
//! ([`crate`]'s `audit::record`) keeps working untouched — it simply leaves the
//! request-context columns `NULL`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// New nullable columns added to `audit_log`, paired with a builder for each.
fn added_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef::new(Alias::new("method")).string().null().take(),
        ColumnDef::new(Alias::new("path")).string().null().take(),
        ColumnDef::new(Alias::new("status_code"))
            .integer()
            .null()
            .take(),
        ColumnDef::new(Alias::new("request_id"))
            .uuid()
            .null()
            .take(),
        ColumnDef::new(Alias::new("ip")).string().null().take(),
        ColumnDef::new(Alias::new("duration_ms"))
            .big_integer()
            .null()
            .take(),
        ColumnDef::new(Alias::new("principal_kind"))
            .string()
            .null()
            .take(),
    ]
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for mut column in added_columns() {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("audit_log"))
                        .add_column_if_not_exists(&mut column)
                        .to_owned(),
                )
                .await?;
        }

        // The request feed is browsed by path and by principal kind, so index both.
        for c in ["path", "principal_kind"] {
            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name(format!("idx_audit_log_{c}"))
                        .table(Alias::new("audit_log"))
                        .col(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for c in ["path", "principal_kind"] {
            manager
                .drop_index(
                    Index::drop()
                        .name(format!("idx_audit_log_{c}"))
                        .table(Alias::new("audit_log"))
                        .to_owned(),
                )
                .await?;
        }
        for name in [
            "method",
            "path",
            "status_code",
            "request_id",
            "ip",
            "duration_ms",
            "principal_kind",
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("audit_log"))
                        .drop_column(Alias::new(name))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
