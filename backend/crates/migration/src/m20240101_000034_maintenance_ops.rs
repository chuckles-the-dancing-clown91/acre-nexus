//! **Maintenance operations round-out** — parts, stock, follow-ups, and
//! resident feedback:
//!
//! * `inventory_item` — the parts/supplies stockroom: SKU, quantity on hand,
//!   unit cost, reorder level, storage location, and a serial-number pool
//!   for serialized stock. Low stock surfaces through the helpdesk scan.
//! * `ticket_line` — itemized parts / labor / fees on a work order; totals
//!   drive the ticket's cost. A part line can consume inventory (and a
//!   serial) and restocks if removed.
//! * `maintenance_ticket` gains the **waiting-on** discipline (`waiting_on`
//!   and `follow_up_date`, enforced with a follow-up note when a ticket
//!   goes on hold; the scan reminds when the date arrives) and the resident
//!   **review** (`rating` 1–5, `review_comment`, `reviewed_at`).
//!
//! Tenant-owned, with the same enforced RLS as every other scoped table.

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
                    .table(Alias::new("inventory_item"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    // NULL = shared/company-wide stock; set = kept on site.
                    .col(col("property_id").uuid().null())
                    .col(col("name").string().not_null())
                    .col(col("sku").string().null())
                    // part | material | tool | supply | other
                    .col(col("category").string().not_null().default("part"))
                    .col(col("quantity").integer().not_null().default(0))
                    .col(col("unit_cost_cents").big_integer().null())
                    // Alert when quantity falls to/below this (0 = never).
                    .col(col("reorder_level").integer().not_null().default(0))
                    .col(col("storage_location").string().null())
                    // Serial-number pool for serialized stock (JSON array of
                    // strings); consuming a part takes one out.
                    .col(col("serial_numbers").json_binary().not_null().default("[]"))
                    .col(col("notes").text().null())
                    // Set while an un-restocked low-stock alert is out; the
                    // scan re-arms it when quantity rises above the level.
                    .col(
                        col("low_stock_alerted_at")
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    // active | archived
                    .col(col("status").string().not_null().default("active"))
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "inventory_item", "tenant_id").await?;
        index(manager, "inventory_item", "property_id").await?;
        enforce_rls(manager, "inventory_item").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("ticket_line"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("ticket_id").uuid().not_null())
                    // part | labor | fee | other
                    .col(col("kind").string().not_null().default("part"))
                    .col(col("description").string().not_null())
                    // Set when the part came out of inventory.
                    .col(col("inventory_item_id").uuid().null())
                    .col(col("serial_number").string().null())
                    .col(col("quantity").integer().not_null().default(1))
                    .col(col("unit_cost_cents").big_integer().not_null().default(0))
                    .col(col("total_cents").big_integer().not_null().default(0))
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "ticket_line", "tenant_id").await?;
        index(manager, "ticket_line", "ticket_id").await?;
        enforce_rls(manager, "ticket_line").await?;

        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE maintenance_ticket \
               ADD COLUMN IF NOT EXISTS waiting_on varchar NULL, \
               ADD COLUMN IF NOT EXISTS follow_up_date varchar NULL, \
               ADD COLUMN IF NOT EXISTS rating integer NULL, \
               ADD COLUMN IF NOT EXISTS review_comment text NULL, \
               ADD COLUMN IF NOT EXISTS reviewed_at timestamptz NULL;",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE maintenance_ticket \
               DROP COLUMN IF EXISTS reviewed_at, \
               DROP COLUMN IF EXISTS review_comment, \
               DROP COLUMN IF EXISTS rating, \
               DROP COLUMN IF EXISTS follow_up_date, \
               DROP COLUMN IF EXISTS waiting_on;",
        )
        .await?;
        for table in ["ticket_line", "inventory_item"] {
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
