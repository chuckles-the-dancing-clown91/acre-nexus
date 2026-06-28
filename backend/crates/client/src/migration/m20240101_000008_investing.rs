//! Client-side of the investor-onboarding schema: the counterparty/entities
//! registry (banks, lenders, contractors …) and their notes.
//!
//! `counterparty_note.author_user_id` references a user in the **user** database;
//! `mortgage.lender_id` / `ownership` / `lien` in the **property** database point
//! back at these counterparties. All such links are plain `Uuid` columns
//! enforced by the application layer, never DB foreign keys.

use super::{col, index, ts, uuid_pk};
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000008_client_investing"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- counterparty (entities registry) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("counterparty"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("kind").string().not_null().default("other"))
                    .col(col("name").string().not_null())
                    .col(col("contact_name").string().null())
                    .col(col("email").string().null())
                    .col(col("phone").string().null())
                    .col(col("website").string().null())
                    .col(col("address").string().null())
                    .col(col("notes").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "counterparty", "tenant_id").await?;

        // ---- counterparty_note ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("counterparty_note"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("counterparty_id").uuid().not_null())
                    .col(col("author_user_id").uuid().null())
                    .col(col("body").text().not_null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "counterparty_note", "counterparty_id").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in ["counterparty_note", "counterparty"] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
