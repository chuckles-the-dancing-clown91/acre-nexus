//! Adds the `tenant_module` table: per-tenant overrides for which pluggable
//! platform modules are enabled. A unique index on `(tenant_id, module_key)`
//! guarantees a single override row per module per tenant.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("tenant_module"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .uuid()
                            .not_null()
                            .primary_key()
                            .take(),
                    )
                    .col(ColumnDef::new(Alias::new("tenant_id")).uuid().not_null().take())
                    .col(ColumnDef::new(Alias::new("module_key")).string().not_null().take())
                    .col(
                        ColumnDef::new(Alias::new("enabled"))
                            .boolean()
                            .not_null()
                            .default(true)
                            .take(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp())
                            .take(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq_tenant_module")
                    .table(Alias::new("tenant_module"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("module_key"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Alias::new("tenant_module")).to_owned())
            .await
    }
}
