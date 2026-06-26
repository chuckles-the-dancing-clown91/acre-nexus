//! Initial schema: all core tables for the shared-schema multi-tenant model.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Shorthand for a `TIMESTAMPTZ NOT NULL DEFAULT now()` column.
fn ts(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name))
        .timestamp_with_time_zone()
        .not_null()
        .default(Expr::current_timestamp())
        .take()
}

fn uuid_pk() -> ColumnDef {
    ColumnDef::new(Alias::new("id"))
        .uuid()
        .not_null()
        .primary_key()
        .take()
}

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
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
        // ---- tenant ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("tenant"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("slug").string().not_null().unique_key())
                    .col(col("name").string().not_null())
                    .col(col("plan").string().not_null().default("starter"))
                    .col(col("status").string().not_null().default("active"))
                    .col(col("custom_domain").string().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;

        // ---- app_user ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("app_user"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().null())
                    .col(col("email").string().not_null().unique_key())
                    .col(col("password_hash").string().not_null())
                    .col(col("name").string().not_null())
                    .col(col("is_platform_staff").boolean().not_null().default(false))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "app_user", "tenant_id").await?;

        // ---- role ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("role"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().null())
                    .col(col("key").string().not_null())
                    .col(col("name").string().not_null())
                    .col(col("description").string().not_null().default(""))
                    .col(col("is_system").boolean().not_null().default(false))
                    .to_owned(),
            )
            .await?;

        // ---- role_permission ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("role_permission"))
                    .if_not_exists()
                    .col(
                        col("id")
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(col("role_id").uuid().not_null())
                    .col(col("permission").string().not_null())
                    .to_owned(),
            )
            .await?;
        index(manager, "role_permission", "role_id").await?;

        // ---- user_role ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("user_role"))
                    .if_not_exists()
                    .col(
                        col("id")
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(col("user_id").uuid().not_null())
                    .col(col("role_id").uuid().not_null())
                    .col(col("tenant_id").uuid().null())
                    .to_owned(),
            )
            .await?;
        index(manager, "user_role", "user_id").await?;

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

        // ---- api_token ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("api_token"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("prefix").string().not_null())
                    .col(col("token_hash").string().not_null().unique_key())
                    .col(col("scopes").json_binary().not_null())
                    .col(col("last_used_at").timestamp_with_time_zone().null())
                    .col(col("expires_at").timestamp_with_time_zone().null())
                    .col(col("revoked_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "api_token", "tenant_id").await?;

        // ---- theme ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("theme"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null().unique_key())
                    .col(col("company_name").string().not_null())
                    .col(col("logo_url").string().null())
                    .col(col("primary_color").string().not_null().default("#F5451F"))
                    .col(col("accent_color").string().not_null().default("#F5451F"))
                    .col(col("default_mode").string().not_null().default("light"))
                    .col(col("legal_templates").json_binary().not_null())
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;

        // ---- refresh_token ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("refresh_token"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("user_id").uuid().not_null())
                    .col(col("token_hash").string().not_null().unique_key())
                    .col(col("expires_at").timestamp_with_time_zone().not_null())
                    .col(col("revoked_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "refresh_token", "user_id").await?;

        // ---- background_job ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("background_job"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("kind").string().not_null())
                    .col(col("status").string().not_null().default("pending"))
                    .col(col("payload").json_binary().not_null())
                    .col(col("result").json_binary().null())
                    .col(ts("run_at"))
                    .col(col("attempts").integer().not_null().default(0))
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "background_job", "status").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in [
            "background_job",
            "refresh_token",
            "theme",
            "api_token",
            "application",
            "listing",
            "property",
            "llc",
            "user_role",
            "role_permission",
            "role",
            "app_user",
            "tenant",
        ] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
