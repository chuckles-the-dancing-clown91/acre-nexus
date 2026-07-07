//! **Full maintenance system round-out** — the remaining pieces of a
//! complete work-order product:
//!
//! * `asset` — the equipment registry: AC units, water heaters, appliances
//!   and other serviceable utilities per property (optionally per unit),
//!   with make/model/serial and warranty tracking. Tickets reference the
//!   asset being serviced; manuals/photos ride the document service
//!   (`owner_type = "asset"`).
//! * `maintenance_ticket` gains the intake context a dispatcher needs:
//!   `location` (where in the home), `access_notes` + `permission_to_enter`
//!   (how to get in), and `asset_id` (what's being serviced).
//! * `ticket_comment` gains `visibility` (`public` residents see it /
//!   `internal` staff-only notes) and `author_name` for display.
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
                    .table(Alias::new("asset"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("unit_id").uuid().null())
                    // hvac | appliance | plumbing | electrical | safety |
                    // structural | other
                    .col(col("kind").string().not_null().default("other"))
                    // Display name, e.g. "AC — living room".
                    .col(col("name").string().not_null())
                    .col(col("make").string().null())
                    .col(col("model").string().null())
                    .col(col("serial_number").string().null())
                    // ISO dates.
                    .col(col("install_date").string().null())
                    .col(col("warranty_expires").string().null())
                    .col(col("notes").text().null())
                    // active | retired
                    .col(col("status").string().not_null().default("active"))
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "asset", "tenant_id").await?;
        index(manager, "asset", "property_id").await?;
        enforce_rls(manager, "asset").await?;

        // Intake context on the work order itself.
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE maintenance_ticket \
               ADD COLUMN IF NOT EXISTS location varchar NULL, \
               ADD COLUMN IF NOT EXISTS access_notes text NULL, \
               ADD COLUMN IF NOT EXISTS permission_to_enter boolean NOT NULL DEFAULT false, \
               ADD COLUMN IF NOT EXISTS asset_id uuid NULL;",
        )
        .await?;

        // Public replies vs. internal notes + display names on the timeline.
        db.execute_unprepared(
            "ALTER TABLE ticket_comment \
               ADD COLUMN IF NOT EXISTS visibility varchar NOT NULL DEFAULT 'public', \
               ADD COLUMN IF NOT EXISTS author_name varchar NULL;",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE ticket_comment \
               DROP COLUMN IF EXISTS author_name, \
               DROP COLUMN IF EXISTS visibility;",
        )
        .await?;
        db.execute_unprepared(
            "ALTER TABLE maintenance_ticket \
               DROP COLUMN IF EXISTS asset_id, \
               DROP COLUMN IF EXISTS permission_to_enter, \
               DROP COLUMN IF EXISTS access_notes, \
               DROP COLUMN IF EXISTS location;",
        )
        .await?;
        db.execute_unprepared("DROP POLICY IF EXISTS asset_tenant_isolation ON asset;")
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("asset"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
