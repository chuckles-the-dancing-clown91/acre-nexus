//! Rental operations + title schema:
//! * **Rentals** — `unit` (rentable spaces), `lease` (tenancies with rental +
//!   payment status), `lease_payment` (rent ledger).
//! * **Maintenance** — `maintenance_ticket` (work orders, assignable to a user
//!   or a contractor) + `ticket_comment` (timeline).
//! * **Title** — `ownership` (deed holders) and `lien` (encumbrances).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

fn uuid_pk() -> ColumnDef {
    ColumnDef::new(Alias::new("id"))
        .uuid()
        .not_null()
        .primary_key()
        .take()
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
        // ---- unit ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("unit"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("unit_number").string().not_null().default(""))
                    .col(col("beds").integer().null())
                    .col(col("baths").double().null())
                    .col(col("sqft").integer().null())
                    .col(col("market_rent_cents").big_integer().null())
                    .col(col("status").string().not_null().default("vacant"))
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "unit", "property_id").await?;

        // ---- lease ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("lease"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("unit_id").uuid().null())
                    .col(col("tenant_name").string().not_null())
                    .col(col("tenant_email").string().null())
                    .col(col("tenant_phone").string().null())
                    .col(col("rent_cents").big_integer().not_null().default(0))
                    .col(col("deposit_cents").big_integer().null())
                    .col(col("start_date").string().not_null().default(""))
                    .col(col("end_date").string().null())
                    .col(col("status").string().not_null().default("active"))
                    .col(col("payment_status").string().not_null().default("current"))
                    .col(col("balance_cents").big_integer().not_null().default(0))
                    .col(col("notes").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "lease", "property_id").await?;

        // ---- lease_payment ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("lease_payment"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().not_null())
                    .col(col("due_date").string().not_null().default(""))
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("paid_date").string().null())
                    .col(col("status").string().not_null().default("due"))
                    .col(col("method").string().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "lease_payment", "lease_id").await?;

        // ---- maintenance_ticket ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("maintenance_ticket"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("unit_id").uuid().null())
                    .col(col("lease_id").uuid().null())
                    .col(col("title").string().not_null())
                    .col(col("description").text().null())
                    .col(col("category").string().not_null().default("general"))
                    .col(col("priority").string().not_null().default("normal"))
                    .col(col("status").string().not_null().default("open"))
                    .col(col("assignee_user_id").uuid().null())
                    .col(col("assignee_entity_id").uuid().null())
                    .col(col("reporter").string().null())
                    .col(col("due_date").string().null())
                    .col(col("cost_cents").big_integer().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "maintenance_ticket", "property_id").await?;
        index(manager, "maintenance_ticket", "status").await?;

        // ---- ticket_comment ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("ticket_comment"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("ticket_id").uuid().not_null())
                    .col(col("author_user_id").uuid().null())
                    .col(col("kind").string().not_null().default("comment"))
                    .col(col("body").text().not_null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "ticket_comment", "ticket_id").await?;

        // ---- ownership ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("ownership"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("owner_kind").string().not_null().default("llc"))
                    .col(col("owner_id").uuid().null())
                    .col(col("owner_name").string().not_null())
                    .col(col("vesting").string().null())
                    .col(col("percent_bps").integer().not_null().default(10000))
                    .col(col("deed_type").string().null())
                    .col(col("deed_recorded_date").string().null())
                    .col(col("deed_reference").string().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "ownership", "property_id").await?;

        // ---- lien ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("lien"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("lienholder_id").uuid().null())
                    .col(col("lienholder_name").string().not_null())
                    .col(col("kind").string().not_null().default("other"))
                    .col(col("amount_cents").big_integer().null())
                    .col(col("position").integer().null())
                    .col(col("recorded_date").string().null())
                    .col(col("status").string().not_null().default("active"))
                    .col(col("reference").string().null())
                    .col(col("notes").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "lien", "property_id").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in [
            "lien",
            "ownership",
            "ticket_comment",
            "maintenance_ticket",
            "lease_payment",
            "lease",
            "unit",
        ] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
