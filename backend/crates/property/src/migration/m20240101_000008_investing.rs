//! Property-side of the investor-onboarding schema: investor columns on
//! `property`, financing (`mortgage`) and the per-property investment workflow
//! (`workflow_event`). The counterparty/entities registry moved to the **client**
//! database (see `acre_client`'s `m20240101_000008_investing`).
//!
//! Note: `mortgage.lender_id` and `ownership`/`lien` lienholder ids reference
//! counterparties that now live in a *different* database — they are plain
//! `Uuid` columns enforced by the application layer, never DB foreign keys.

use super::{col, index, ts, uuid_pk};
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000008_property_investing"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- property: investor fields ----
        for mut c in [
            col("property_type").string().not_null().default("").take(),
            col("strategy").string().not_null().default("rental").take(),
            col("workflow_stage").string().not_null().default("").take(),
            col("purchase_price_cents").big_integer().null().take(),
            col("acquired_on").string().null().take(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("property"))
                        .add_column_if_not_exists(&mut c)
                        .to_owned(),
                )
                .await?;
        }
        index(manager, "property", "strategy").await?;

        // ---- mortgage ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("mortgage"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("lender_id").uuid().null())
                    .col(col("kind").string().not_null().default("purchase"))
                    .col(col("position").integer().not_null().default(1))
                    .col(col("original_amount_cents").big_integer().null())
                    .col(col("current_balance_cents").big_integer().null())
                    .col(col("interest_rate_bps").integer().null())
                    .col(col("term_months").integer().null())
                    .col(col("monthly_payment_cents").big_integer().null())
                    .col(col("escrow_monthly_cents").big_integer().null())
                    .col(col("start_date").string().null())
                    .col(col("maturity_date").string().null())
                    .col(col("loan_number").string().null())
                    .col(col("status").string().not_null().default("active"))
                    .col(col("notes").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "mortgage", "property_id").await?;

        // ---- workflow_event ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("workflow_event"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("strategy").string().not_null().default("rental"))
                    .col(col("from_stage").string().null())
                    .col(col("to_stage").string().not_null())
                    .col(col("note").text().null())
                    .col(col("actor_user_id").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "workflow_event", "property_id").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in ["workflow_event", "mortgage"] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        for c in [
            "property_type",
            "strategy",
            "workflow_stage",
            "purchase_price_cents",
            "acquired_on",
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("property"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
