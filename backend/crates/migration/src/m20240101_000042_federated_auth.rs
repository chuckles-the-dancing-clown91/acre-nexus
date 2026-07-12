//! **Federated login + MFA** (issue #63).
//!
//! * `federated_identity` — links an OAuth/OIDC provider account
//!   (`provider` + `subject`) to an `app_user`, so "Log in with Google /
//!   Microsoft / Apple" maps onto the existing identity model without
//!   disturbing it. One user may link several providers; a `(provider,
//!   subject)` pair is globally unique.
//! * `user_totp` — a user's TOTP (authenticator-app) MFA enrolment: the shared
//!   secret **sealed** (AES-256-GCM, never plaintext) plus an `enabled` flag.
//!
//! Both key on `user_id` with **no** `tenant_id` and therefore carry no RLS —
//! exactly like `refresh_token`, because they must be readable during login,
//! before any tenant context (the `app.tenant_id` GUC) is set.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

fn ts(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name))
        .timestamp_with_time_zone()
        .not_null()
        .default(Expr::current_timestamp())
        .take()
}

async fn index(manager: &SchemaManager<'_>, table: &str, column: &str) -> Result<(), DbErr> {
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

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("federated_identity"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("user_id").uuid().not_null())
                    // google | microsoft | apple (the provider key).
                    .col(col("provider").string().not_null())
                    // The provider's stable subject identifier (OIDC `sub`).
                    .col(col("subject").string().not_null())
                    // The email the provider asserted at link time.
                    .col(col("email").string().not_null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "federated_identity", "user_id").await?;
        // A provider account maps to exactly one app_user.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .unique()
                    .name("idx_federated_identity_provider_subject")
                    .table(Alias::new("federated_identity"))
                    .col(Alias::new("provider"))
                    .col(Alias::new("subject"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("user_totp"))
                    .if_not_exists()
                    // 1:1 with app_user.
                    .col(col("user_id").uuid().not_null().primary_key())
                    // The base32 TOTP secret, sealed under the PII key.
                    .col(col("secret_ciphertext").string().not_null())
                    .col(col("secret_nonce").string().not_null())
                    // Enrolment is two-step: a secret is stored, then confirmed
                    // with a valid code before it's `enabled` (challenged at login).
                    .col(col("enabled").boolean().not_null().default(false))
                    .col(col("confirmed_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("user_totp")).if_exists().to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("federated_identity"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
