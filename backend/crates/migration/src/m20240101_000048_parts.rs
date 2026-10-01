//! **Appliances, parts and stock** (Vantedge phase 2C).
//!
//! * `asset` gains what it cost, how long it should last, and its warranty.
//! * `asset_part` — the parts catalog: which stock items fit an appliance.
//! * `ticket_part` — a work order's parts list: potential parts, what a
//!   finding needs, the shopping list, and the close-out state (from stock /
//!   ordered / pick up / received / used) — one row moves through all of it.
//! * `inventory_item` gains a barcode, a unit and a vendor;
//!   `inventory_movement` records every receive / use / count with the cost
//!   it moved at (weighted-average unit cost lives on the item).
//! * `maintenance_plan` can be about one appliance.
//!
//! New tables are tenant-owned, with the same enforced RLS as every other
//! scoped table.

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

/// The isolation predicate, empty-GUC safe (see migration `000038`).
const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

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

const TABLES: &[&str] = &["inventory_movement", "ticket_part", "asset_part"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Appliances: what they cost, how long they last, who covers them.
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("asset"))
                    .add_column(col("location").string().null())
                    .add_column(col("purchased_on").string().null())
                    .add_column(col("purchase_price_cents").big_integer().null())
                    .add_column(col("expected_life_years").integer().null())
                    .add_column(col("warranty_provider").string().null())
                    .add_column(col("warranty_notes").string().null())
                    .to_owned(),
            )
            .await?;
        // Stock: a barcode to scan, a unit, and who sells it.
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("inventory_item"))
                    .add_column(col("barcode").string().null())
                    .add_column(col("unit").string().not_null().default("ea"))
                    .add_column(col("vendor").string().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .unique()
                    .name("uq_inventory_item_tenant_barcode")
                    .table(Alias::new("inventory_item"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("barcode"))
                    .to_owned(),
            )
            .await?;
        // Routine maintenance on a specific appliance.
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("maintenance_plan"))
                    .add_column(col("asset_id").uuid().null())
                    .to_owned(),
            )
            .await?;
        // Which stock items fit which appliance.
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("asset_part"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("asset_id").uuid().not_null())
                    .col(col("inventory_item_id").uuid().not_null())
                    .col(col("quantity").integer().not_null().default(1))
                    // filter | belt | element | capacitor | … free text
                    .col(col("role").string().null())
                    .col(col("note").string().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "asset_part", "asset_id").await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .unique()
                    .name("uq_asset_part_item")
                    .table(Alias::new("asset_part"))
                    .col(Alias::new("asset_id"))
                    .col(Alias::new("inventory_item_id"))
                    .to_owned(),
            )
            .await?;
        // A work order's parts: potential, needed, on the shopping list, on
        // order, received, used — one row moves through all of it.
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("ticket_part"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("ticket_id").uuid().not_null())
                    .col(col("inventory_item_id").uuid().null())
                    .col(col("name").string().not_null())
                    .col(col("quantity").integer().not_null().default(1))
                    // potential | needed | from_stock | to_order | ordered | pick_up | received | used | skipped
                    .col(col("status").string().not_null().default("potential"))
                    // asset | finding | plan | typed
                    .col(col("source").string().not_null().default("typed"))
                    .col(col("finding_comment_id").uuid().null())
                    .col(col("need_by").string().null())
                    // property | office | other
                    .col(col("ship_to").string().null())
                    .col(col("ship_to_note").string().null())
                    .col(col("vendor").string().null())
                    .col(col("tracking").string().null())
                    .col(col("unit_cost_cents").big_integer().null())
                    .col(col("expense_id").uuid().null())
                    .col(col("ticket_line_id").uuid().null())
                    .col(col("note").string().null())
                    .col(col("ordered_at").timestamp_with_time_zone().null())
                    .col(col("ordered_by").uuid().null())
                    .col(col("received_at").timestamp_with_time_zone().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "ticket_part", "ticket_id").await?;
        index(manager, "ticket_part", "status").await?;
        // Every change to stock, with the cost it moved at.
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("inventory_movement"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("inventory_item_id").uuid().not_null())
                    // receive | use | count | restock
                    .col(col("kind").string().not_null())
                    // signed change
                    .col(col("quantity").integer().not_null())
                    .col(col("unit_cost_cents").big_integer().not_null().default(0))
                    .col(col("ticket_id").uuid().null())
                    .col(col("expense_id").uuid().null())
                    .col(col("note").string().null())
                    .col(col("recorded_by").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "inventory_movement", "inventory_item_id").await?;
        index(manager, "inventory_movement", "ticket_id").await?;
        for t in TABLES {
            enforce_rls(manager, t).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in TABLES {
            manager
                .drop_table(Table::drop().table(Alias::new(*t)).if_exists().to_owned())
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("maintenance_plan"))
                    .drop_column(Alias::new("asset_id"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("inventory_item"))
                    .drop_column(Alias::new("barcode"))
                    .drop_column(Alias::new("unit"))
                    .drop_column(Alias::new("vendor"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("asset"))
                    .drop_column(Alias::new("location"))
                    .drop_column(Alias::new("purchased_on"))
                    .drop_column(Alias::new("purchase_price_cents"))
                    .drop_column(Alias::new("expected_life_years"))
                    .drop_column(Alias::new("warranty_provider"))
                    .drop_column(Alias::new("warranty_notes"))
                    .to_owned(),
            )
            .await
    }
}
