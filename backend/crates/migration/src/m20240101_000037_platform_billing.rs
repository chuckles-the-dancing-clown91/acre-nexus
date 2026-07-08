//! **SaaS platform billing** (roadmap Phase 8) — Acre HQ billing its client
//! workspaces for their subscription:
//!
//! * `platform_invoice` — one bill per tenant per billing month (plan base fee
//!   + metered per-unit overage), moving `draft → open → paid` (or `void`).
//! * `platform_invoice_line` — the frozen line items behind an invoice total.
//!
//! Both are tenant-scoped with enforced RLS, like every other scoped table; the
//! platform plane (null tenant GUC) authors them across every workspace.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
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

const RLS_PRED: &str = "current_setting('app.tenant_id', true) IS NULL \
     OR tenant_id::text = current_setting('app.tenant_id', true)";

async fn enforce_rls(manager: &SchemaManager<'_>, table: &str) -> Result<(), DbErr> {
    let policy = format!("{table}_tenant_isolation");
    let sql = format!(
        "ALTER TABLE {table} ENABLE ROW LEVEL SECURITY; \
         ALTER TABLE {table} FORCE ROW LEVEL SECURITY; \
         DROP POLICY IF EXISTS {policy} ON {table}; \
         CREATE POLICY {policy} ON {table} \
           USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
    );
    manager.get_connection().execute_unprepared(&sql).await?;
    Ok(())
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("platform_invoice"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("period").string().not_null())
                    .col(col("plan").string().not_null().default("starter"))
                    .col(col("unit_count").integer().not_null().default(0))
                    .col(col("included_units").integer().not_null().default(0))
                    .col(col("base_cents").big_integer().not_null().default(0))
                    .col(col("overage_cents").big_integer().not_null().default(0))
                    .col(col("total_cents").big_integer().not_null().default(0))
                    .col(col("status").string().not_null().default("draft"))
                    .col(col("issued_at").timestamp_with_time_zone().null())
                    .col(col("due_date").string().null())
                    .col(col("paid_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "platform_invoice", "tenant_id").await?;
        // One invoice per tenant per period keeps generation idempotent.
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_platform_invoice_tenant_period")
                    .table(Alias::new("platform_invoice"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("period"))
                    .unique()
                    .to_owned(),
            )
            .await?;
        enforce_rls(manager, "platform_invoice").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("platform_invoice_line"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("invoice_id").uuid().not_null())
                    .col(col("description").string().not_null())
                    .col(col("quantity").integer().not_null().default(1))
                    .col(col("unit_price_cents").big_integer().not_null().default(0))
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("sort_order").integer().not_null().default(0))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "platform_invoice_line", "tenant_id").await?;
        index(manager, "platform_invoice_line", "invoice_id").await?;
        enforce_rls(manager, "platform_invoice_line").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in ["platform_invoice_line", "platform_invoice"] {
            db.execute_unprepared(&format!(
                "DROP POLICY IF EXISTS {table}_tenant_isolation ON {table};"
            ))
            .await?;
            manager
                .drop_table(
                    Table::drop()
                        .table(Alias::new(table))
                        .if_exists()
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
