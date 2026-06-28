//! Property-intelligence schema: the rich per-property data the enrichment engine
//! populates (parcel/county detail, tax history, AVM valuations, schools,
//! utilities) plus the `enrichment_run` audit trail. Also upgrades
//! `background_job` into a proper retrying queue (`max_attempts`, `last_error`).

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
        // ---- background_job: retry budget + last error ----
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

        // ---- property_detail (1:1) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("property_detail"))
                    .if_not_exists()
                    .col(col("property_id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("beds").integer().null())
                    .col(col("baths").double().null())
                    .col(col("sqft").integer().null())
                    .col(col("lot_size_sqft").big_integer().null())
                    .col(col("property_type").string().null())
                    .col(col("stories").integer().null())
                    .col(col("parking_spaces").integer().null())
                    .col(col("heating").string().null())
                    .col(col("cooling").string().null())
                    .col(col("latitude").double().null())
                    .col(col("longitude").double().null())
                    .col(col("geocode_accuracy").string().null())
                    .col(col("matched_address").string().null())
                    .col(col("apn").string().null())
                    .col(col("legal_description").text().null())
                    .col(col("zoning").string().null())
                    .col(col("subdivision").string().null())
                    .col(col("county").string().null())
                    .col(col("fips").string().null())
                    .col(col("owner_of_record").string().null())
                    .col(col("last_sale_date").string().null())
                    .col(col("last_sale_price_cents").big_integer().null())
                    .col(col("flood_zone").string().null())
                    .col(col("walk_score").integer().null())
                    .col(col("last_enriched_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "property_detail", "tenant_id").await?;

        // ---- property_tax ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("property_tax"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("tax_year").integer().not_null())
                    .col(col("assessed_value_cents").big_integer().null())
                    .col(col("land_value_cents").big_integer().null())
                    .col(col("improvement_value_cents").big_integer().null())
                    .col(col("tax_amount_cents").big_integer().null())
                    .col(col("tax_rate_bps").integer().null())
                    .col(col("source").string().not_null().default("simulated"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "property_tax", "property_id").await?;

        // ---- property_valuation ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("property_valuation"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("as_of").string().not_null())
                    .col(col("estimated_value_cents").big_integer().null())
                    .col(col("value_low_cents").big_integer().null())
                    .col(col("value_high_cents").big_integer().null())
                    .col(col("estimated_rent_cents").big_integer().null())
                    .col(col("confidence").integer().null())
                    .col(col("source").string().not_null().default("simulated"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "property_valuation", "property_id").await?;

        // ---- property_school ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("property_school"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("level").string().not_null().default(""))
                    .col(col("district").string().null())
                    .col(col("rating").integer().null())
                    .col(col("distance_mi").double().null())
                    .col(col("grades").string().null())
                    .col(col("source").string().not_null().default("simulated"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "property_school", "property_id").await?;

        // ---- property_utility ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("property_utility"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("utility_type").string().not_null())
                    .col(col("provider").string().not_null().default(""))
                    .col(col("est_monthly_cost_cents").big_integer().null())
                    .col(col("phone").string().null())
                    .col(col("source").string().not_null().default("simulated"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "property_utility", "property_id").await?;

        // ---- enrichment_run ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("enrichment_run"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("source").string().not_null())
                    .col(col("status").string().not_null())
                    .col(col("job_id").uuid().null())
                    .col(col("provider").string().not_null().default("simulated"))
                    .col(col("detail").json_binary().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "enrichment_run", "property_id").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in [
            "enrichment_run",
            "property_utility",
            "property_school",
            "property_valuation",
            "property_tax",
            "property_detail",
        ] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
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
