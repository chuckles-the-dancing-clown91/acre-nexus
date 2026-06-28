//! Initial schema for the **property** database: the holding entity (`llc`), the
//! `property` asset, and public `listing`s. (Investor columns on `property`,
//! financing and workflow are added by `m20240101_000008_investing`; rich
//! property data by `m20240101_000007_property_data`; rentals/title by
//! `m20240101_000009_rentals_title`.)

use super::{col, index, ts, uuid_pk};
use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000001_property_init"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- llc ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("llc"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("ein").string().not_null().default(""))
                    .col(col("state").string().not_null().default(""))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "llc", "tenant_id").await?;

        // ---- property ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("property"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("llc_id").uuid().null())
                    .col(col("name").string().not_null())
                    .col(col("address").string().not_null().default(""))
                    .col(col("city").string().not_null().default(""))
                    .col(col("units").integer().not_null().default(0))
                    .col(col("occupied_units").integer().not_null().default(0))
                    .col(
                        col("monthly_rent_cents")
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(col("status").string().not_null().default("Stabilized"))
                    .col(col("year_built").integer().not_null().default(0))
                    .col(col("manager").string().not_null().default(""))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "property", "tenant_id").await?;

        // ---- listing ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("listing"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().null())
                    .col(col("title").string().not_null())
                    .col(col("address").string().not_null().default(""))
                    .col(col("city").string().not_null().default(""))
                    .col(col("beds").integer().not_null().default(0))
                    .col(col("baths").integer().not_null().default(0))
                    .col(col("sqft").integer().not_null().default(0))
                    .col(col("rent_cents").big_integer().not_null().default(0))
                    .col(col("status").string().not_null().default("Available"))
                    .col(col("available_on").string().not_null().default("Now"))
                    .col(col("description").text().not_null().default(""))
                    .col(col("is_public").boolean().not_null().default(true))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "listing", "tenant_id").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in ["listing", "property", "llc"] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
