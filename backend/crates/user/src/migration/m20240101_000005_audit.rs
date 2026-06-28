//! Adds the `audit_log` table for security-relevant action history (PII reveals,
//! role/user changes). Indexed by time and actor for the dashboard trail.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("audit_log"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("actor_user_id").uuid().null())
                    .col(col("action").string().not_null())
                    .col(col("target_type").string().null())
                    .col(col("target_id").string().null())
                    .col(col("tenant_id").uuid().null())
                    .col(col("metadata").json_binary().null())
                    .col(
                        col("created_at")
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        for c in ["created_at", "actor_user_id", "action"] {
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
        manager
            .drop_table(Table::drop().table(Alias::new("audit_log")).to_owned())
            .await
    }
}
