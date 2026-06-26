//! IAM build-out: separate user identity from profile (PII), add the permission
//! and persona catalogs, multi-tenant memberships, and a `scope` on roles.
//!
//! * `app_user` gains `username`, `status`, `last_login_at`.
//! * `role` gains `scope` (`platform` | `tenant`).
//! * new `permission` (catalog), `profile_type` (persona catalog),
//!   `user_profile` (1:1 PII; SSN/gov-ID stored as encrypted ciphertext + nonce),
//!   and `membership` (user ↔ platform/tenant with a persona).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

fn pk_uuid() -> ColumnDef {
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
        // ---- alter app_user ----
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("app_user"))
                    .add_column(col("username").string().null())
                    .add_column(col("status").string().not_null().default("active"))
                    .add_column(col("last_login_at").timestamp_with_time_zone().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq_app_user_username")
                    .table(Alias::new("app_user"))
                    .col(Alias::new("username"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        // ---- alter role: add scope ----
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("role"))
                    .add_column(col("scope").string().not_null().default("tenant"))
                    .to_owned(),
            )
            .await?;

        // ---- permission (catalog) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("permission"))
                    .if_not_exists()
                    .col(col("key").string().not_null().primary_key())
                    .col(col("category").string().not_null())
                    .col(col("label").string().not_null())
                    .col(col("description").string().not_null())
                    .col(col("scope").string().not_null())
                    .col(col("is_system").boolean().not_null().default(true))
                    .to_owned(),
            )
            .await?;

        // ---- profile_type (persona catalog) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("profile_type"))
                    .if_not_exists()
                    .col(col("key").string().not_null().primary_key())
                    .col(col("scope").string().not_null())
                    .col(col("label").string().not_null())
                    .col(col("description").string().not_null())
                    .col(col("default_role").string().not_null())
                    .col(col("is_system").boolean().not_null().default(true))
                    .to_owned(),
            )
            .await?;

        // ---- user_profile (1:1 PII) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("user_profile"))
                    .if_not_exists()
                    .col(col("user_id").uuid().not_null().primary_key())
                    .col(col("legal_first_name").string().null())
                    .col(col("legal_middle_name").string().null())
                    .col(col("legal_last_name").string().null())
                    .col(col("preferred_name").string().null())
                    .col(col("date_of_birth").date().null())
                    .col(col("phone").string().null())
                    .col(col("address_line1").string().null())
                    .col(col("address_line2").string().null())
                    .col(col("city").string().null())
                    .col(col("region").string().null())
                    .col(col("postal_code").string().null())
                    .col(col("country").string().null())
                    .col(col("ssn_ciphertext").string().null())
                    .col(col("ssn_nonce").string().null())
                    .col(col("ssn_last4").string().null())
                    .col(col("gov_id_type").string().null())
                    .col(col("gov_id_ciphertext").string().null())
                    .col(col("gov_id_nonce").string().null())
                    .col(col("gov_id_last4").string().null())
                    .col(col("photo_url").string().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_user_profile_user")
                            .from(Alias::new("user_profile"), Alias::new("user_id"))
                            .to(Alias::new("app_user"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // ---- membership (user ↔ scope/persona) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("membership"))
                    .if_not_exists()
                    .col(pk_uuid())
                    .col(col("user_id").uuid().not_null())
                    .col(col("scope").string().not_null())
                    .col(col("tenant_id").uuid().null())
                    .col(col("profile_type").string().not_null())
                    .col(col("title").string().null())
                    .col(col("status").string().not_null().default("active"))
                    .col(col("is_primary").boolean().not_null().default(false))
                    .col(ts("created_at"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_membership_user")
                            .from(Alias::new("membership"), Alias::new("user_id"))
                            .to(Alias::new("app_user"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        index(manager, "membership", "user_id").await?;
        index(manager, "membership", "tenant_id").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in ["membership", "user_profile", "profile_type", "permission"] {
            manager
                .drop_table(Table::drop().table(Alias::new(table)).to_owned())
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("role"))
                    .drop_column(Alias::new("scope"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("app_user"))
                    .drop_column(Alias::new("username"))
                    .drop_column(Alias::new("status"))
                    .drop_column(Alias::new("last_login_at"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
