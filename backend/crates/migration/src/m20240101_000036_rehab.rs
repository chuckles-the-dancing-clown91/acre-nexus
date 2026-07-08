//! **Rehab / construction management** (roadmap Phase 7, issue #40) — the
//! flip/BRRRR renovation domain:
//!
//! * `rehab_project` — the budget container on a property.
//! * `rehab_line` — itemised scope / budget lines.
//! * `rehab_change_order` — approved deltas to the budget.
//! * `rehab_draw` — draw requests against the budget (with progress photos +
//!   docs via the document service, `owner_type = "rehab_draw"`).
//! * `rehab_lien_waiver` — the four statutory waivers captured per draw.
//!
//! Tenant-owned, with enforced RLS like every other scoped table.

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
                    .table(Alias::new("rehab_project"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("status").string().not_null().default("planning"))
                    .col(col("budget_cents").big_integer().not_null().default(0))
                    .col(col("contingency_bps").integer().not_null().default(0))
                    .col(col("start_date").string().null())
                    .col(col("target_end_date").string().null())
                    .col(col("notes").text().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "rehab_project", "tenant_id").await?;
        index(manager, "rehab_project", "property_id").await?;
        enforce_rls(manager, "rehab_project").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("rehab_line"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("project_id").uuid().not_null())
                    .col(col("category").string().not_null())
                    .col(col("description").text().null())
                    .col(col("budget_cents").big_integer().not_null().default(0))
                    .col(col("sort_order").integer().not_null().default(0))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "rehab_line", "tenant_id").await?;
        index(manager, "rehab_line", "project_id").await?;
        enforce_rls(manager, "rehab_line").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("rehab_change_order"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("project_id").uuid().not_null())
                    .col(col("description").string().not_null())
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("status").string().not_null().default("pending"))
                    .col(col("created_by").uuid().null())
                    .col(col("approved_by").uuid().null())
                    .col(ts("created_at"))
                    .col(col("decided_at").timestamp_with_time_zone().null())
                    .to_owned(),
            )
            .await?;
        index(manager, "rehab_change_order", "tenant_id").await?;
        index(manager, "rehab_change_order", "project_id").await?;
        enforce_rls(manager, "rehab_change_order").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("rehab_draw"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("project_id").uuid().not_null())
                    .col(col("number").integer().not_null().default(1))
                    .col(col("title").string().not_null())
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("status").string().not_null().default("requested"))
                    .col(col("contractor_id").uuid().null())
                    .col(col("notes").text().null())
                    .col(col("requested_by").uuid().null())
                    .col(col("approved_by").uuid().null())
                    .col(col("funded_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "rehab_draw", "tenant_id").await?;
        index(manager, "rehab_draw", "project_id").await?;
        enforce_rls(manager, "rehab_draw").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("rehab_lien_waiver"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("draw_id").uuid().not_null())
                    .col(col("project_id").uuid().not_null())
                    .col(col("waiver_type").string().not_null())
                    .col(col("contractor_id").uuid().null())
                    .col(col("contractor_name").string().not_null().default(""))
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("through_date").string().null())
                    .col(col("status").string().not_null().default("generated"))
                    .col(col("document_id").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "rehab_lien_waiver", "tenant_id").await?;
        index(manager, "rehab_lien_waiver", "draw_id").await?;
        enforce_rls(manager, "rehab_lien_waiver").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in [
            "rehab_lien_waiver",
            "rehab_draw",
            "rehab_change_order",
            "rehab_line",
            "rehab_project",
        ] {
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
