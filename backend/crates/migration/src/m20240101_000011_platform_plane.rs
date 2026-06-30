//! Tenancy spec — **Phase B: the platform plane**.
//!
//! Acre staff live on a separate RBAC plane: they are **never tenant members**.
//! Two new (non-tenant-scoped) tables formalize this:
//!
//! * `platform_staff` — the roster of Acre employees on the platform plane. A row
//!   here (plus a platform-scoped role assignment) is what makes a user staff;
//!   they hold no `membership` in any client workspace.
//! * `impersonation_session` — every time staff enter a tenant they do so through
//!   a **time-boxed, reason-logged, revocable** session (default short TTL).
//!   The audit fairing tags impersonated requests with the platform actor.
//!
//! Neither table is tenant-scoped, so no RLS policy applies (they belong to
//! "Acre HQ"); `impersonation_session.tenant_id` records *which* tenant was
//! entered, for the audit trail.

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
        // ---- platform_staff ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("platform_staff"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("user_id").uuid().not_null())
                    .col(col("status").string().not_null().default("active"))
                    .col(ts("created_at"))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_platform_staff_user")
                            .from(Alias::new("platform_staff"), Alias::new("user_id"))
                            .to(Alias::new("app_user"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq_platform_staff_user")
                    .table(Alias::new("platform_staff"))
                    .col(Alias::new("user_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        // ---- impersonation_session ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("impersonation_session"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("platform_staff_id").uuid().not_null())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("reason").string().not_null())
                    .col(col("expires_at").timestamp_with_time_zone().not_null())
                    .col(col("revoked_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "impersonation_session", "platform_staff_id").await?;
        index(manager, "impersonation_session", "tenant_id").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in ["impersonation_session", "platform_staff"] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
