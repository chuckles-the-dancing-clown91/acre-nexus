//! Upgrades `background_job` into a proper retrying queue.
//!
//! Adds the retry budget (`max_attempts`) and the most-recent-failure message
//! (`last_error`) that the Tokio scheduler in the `api` crate relies on. These
//! live in the **user** database alongside `background_job` itself — the columns
//! were previously (and incorrectly) added by a migration in the `property`
//! crate, which ran against `acre_property` where the table does not exist.

use super::col;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("background_job"))
                    .add_column_if_not_exists(col("max_attempts").integer().not_null().default(5))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("background_job"))
                    .add_column_if_not_exists(col("last_error").text().null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for c in ["max_attempts", "last_error"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("background_job"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
