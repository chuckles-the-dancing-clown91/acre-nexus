//! Leasing lifecycle — application → onboarding → lease signing, with **templated
//! lease documents**, **conditional fees / discounts / amenities**, and **vehicle
//! profiles**.
//!
//! * `application` / `lease` gain renter attributes (`has_pet`, `pet_details`,
//!   `is_military`) that drive conditional charges + auto-generated lease verbiage,
//!   and `lease.application_id` links a signed lease back to the application it
//!   came from.
//! * `fee_schedule` — a per-tenant catalog the landlord configures: fees,
//!   discounts, rebates, and amenities, each with a **condition** (`has_pet`,
//!   `is_military`, `has_vehicle`, `always`, or `manual`) and optional lease
//!   `verbiage`. Matching conditions auto-populate a lease's charges.
//! * `lease_charge` — the resolved line items on one lease (rent add-ons, pet
//!   fees, military discounts, garage/amenity charges …); discounts/rebates are
//!   negative cents.
//! * `vehicle` — a tenant/renter's vehicle profile; garage & parking amenities
//!   pull these details into the lease document.
//! * `lease_document` — a generated lease agreement (rendered from the tenant's
//!   `theme.legal_templates` + the lease/charges/vehicles), with a signing state.
//!
//! All new tables are tenant-owned and get an RLS isolation policy.

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

const TENANT_SCOPED: &[&str] = &["fee_schedule", "lease_charge", "vehicle", "lease_document"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- renter attributes on application + lease ----
        for (table, cols) in [
            (
                "application",
                vec![
                    col("has_pet").boolean().not_null().default(false).take(),
                    col("pet_details").text().null().take(),
                    col("is_military").boolean().not_null().default(false).take(),
                ],
            ),
            (
                "lease",
                vec![
                    col("application_id").uuid().null().take(),
                    col("has_pet").boolean().not_null().default(false).take(),
                    col("pet_details").text().null().take(),
                    col("is_military").boolean().not_null().default(false).take(),
                ],
            ),
        ] {
            for mut c in cols {
                manager
                    .alter_table(
                        Table::alter()
                            .table(Alias::new(table))
                            .add_column_if_not_exists(&mut c)
                            .to_owned(),
                    )
                    .await?;
            }
        }

        // ---- fee_schedule (landlord-configured catalog of conditional fees) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("fee_schedule"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("code").string().not_null())
                    // `fee` | `discount` | `rebate` | `amenity`
                    .col(col("kind").string().not_null().default("fee"))
                    .col(col("label").string().not_null())
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("recurring").boolean().not_null().default(true))
                    // `manual` | `always` | `has_pet` | `is_military` | `has_vehicle`
                    .col(col("condition_type").string().not_null().default("manual"))
                    .col(col("verbiage").text().null())
                    .col(col("active").boolean().not_null().default(true))
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "fee_schedule", "tenant_id").await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq_fee_schedule_tenant_code")
                    .table(Alias::new("fee_schedule"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("code"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        // ---- lease_charge (resolved line items on a lease) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("lease_charge"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().not_null())
                    .col(col("kind").string().not_null().default("fee"))
                    // Optional fee_schedule.code this charge came from.
                    .col(col("code").string().null())
                    .col(col("label").string().not_null())
                    // Positive for charges; negative for discounts/rebates.
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("recurring").boolean().not_null().default(true))
                    // `manual` | `auto` (from a condition) | `application`
                    .col(col("source").string().not_null().default("manual"))
                    .col(col("verbiage").text().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "lease_charge", "tenant_id").await?;
        index(manager, "lease_charge", "lease_id").await?;

        // ---- vehicle (vehicle profile) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("vehicle"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().null())
                    .col(col("application_id").uuid().null())
                    .col(col("user_id").uuid().null())
                    .col(col("make").string().not_null())
                    .col(col("model").string().not_null())
                    .col(col("year").integer().null())
                    .col(col("color").string().null())
                    .col(col("license_plate").string().null())
                    .col(col("plate_state").string().null())
                    .col(col("notes").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "vehicle", "tenant_id").await?;
        index(manager, "vehicle", "lease_id").await?;
        index(manager, "vehicle", "application_id").await?;

        // ---- lease_document (generated, signable agreement) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("lease_document"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().not_null())
                    .col(col("title").string().not_null().default("Residential Lease Agreement"))
                    .col(col("body").text().not_null())
                    .col(col("format").string().not_null().default("text"))
                    // `draft` | `sent` | `signed`
                    .col(col("status").string().not_null().default("draft"))
                    .col(col("generated_at").timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(col("signed_at").timestamp_with_time_zone().null())
                    .col(col("signed_by").string().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "lease_document", "tenant_id").await?;
        index(manager, "lease_document", "lease_id").await?;

        // ---- RLS ----
        let db = manager.get_connection();
        for t in TENANT_SCOPED {
            let sql = format!(
                "ALTER TABLE {t} ENABLE ROW LEVEL SECURITY; \
                 DROP POLICY IF EXISTS {t}_tenant_isolation ON {t}; \
                 CREATE POLICY {t}_tenant_isolation ON {t} \
                   USING (\
                     current_setting('app.tenant_id', true) IS NULL \
                     OR tenant_id::text = current_setting('app.tenant_id', true)\
                   );"
            );
            db.execute_unprepared(&sql).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for t in TENANT_SCOPED {
            let sql = format!(
                "DROP POLICY IF EXISTS {t}_tenant_isolation ON {t}; \
                 ALTER TABLE {t} DISABLE ROW LEVEL SECURITY;"
            );
            db.execute_unprepared(&sql).await?;
        }
        for t in ["lease_document", "vehicle", "lease_charge", "fee_schedule"] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        for c in ["application_id", "has_pet", "pet_details", "is_military"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("lease"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        for c in ["has_pet", "pet_details", "is_military"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("application"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
