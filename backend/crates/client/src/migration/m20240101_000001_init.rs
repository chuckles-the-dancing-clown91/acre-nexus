//! Initial schema for the **client** database: inbound rental `application`s.
//! (The counterparty registry + notes are added by
//! `m20240101_000008_investing`.)
//!
//! `application.listing_id` references a listing that lives in the **property**
//! database — a plain `Uuid` column enforced by the application layer.

use super::{col, index, ts, uuid_pk};
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000001_client_init"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- application ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("application"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("listing_id").uuid().null())
                    .col(col("applicant_name").string().not_null())
                    .col(col("email").string().not_null().default(""))
                    .col(col("phone").string().not_null().default(""))
                    .col(
                        col("annual_income_cents")
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(col("credit_score").integer().null())
                    .col(col("status").string().not_null().default("New"))
                    .col(col("move_in").string().not_null().default(""))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "application", "tenant_id").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("application"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
