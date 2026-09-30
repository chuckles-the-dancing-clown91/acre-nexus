//! **Password links** — set-your-password invites and forgot-password resets.
//!
//! `password_token` holds one-time links: `purpose` is `invite` (7 days, sent
//! when a member is invited) or `reset` (24 hours, "Forgot your password?").
//! Only a SHA-256 hash of the secret is stored, a link works once (`used_at`),
//! and — like `refresh_token` / `user_totp` — the table keys on `user_id` with
//! no `tenant_id` and no RLS, because it is read before anyone is signed in.

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
                    .table(Alias::new("password_token"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("user_id").uuid().not_null())
                    // invite | reset
                    .col(col("purpose").string().not_null())
                    .col(col("token_hash").string().not_null().unique_key())
                    .col(col("expires_at").timestamp_with_time_zone().not_null())
                    .col(col("used_at").timestamp_with_time_zone().null())
                    .col(
                        col("created_at")
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_password_token_user_id")
                    .table(Alias::new("password_token"))
                    .col(Alias::new("user_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("password_token"))
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
