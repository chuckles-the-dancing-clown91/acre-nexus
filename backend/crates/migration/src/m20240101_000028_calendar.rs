//! **Calendar / scheduling / reminders engine** (issue #54).
//!
//! * `reminder` — a generic scheduled event: a subject (`lease` renewal,
//!   `license` / `insurance` expiry, `tour`, `inspection`, or `custom`), a due
//!   date, the lead times (in days) at which to notify, and optional external
//!   recipients. A per-tenant `reminder_scan` job (the `billing_cycle`
//!   pattern) fires notifications through the Phase 1 substrate at each lead
//!   time and records which leads have fired.
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
                    .table(Alias::new("reminder"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    // lease | license | insurance | tour | inspection | custom
                    .col(col("subject_type").string().not_null())
                    // The subject row (lease id, …) when one exists.
                    .col(col("subject_id").uuid().null())
                    .col(col("title").string().not_null())
                    .col(col("description").text().null())
                    // `YYYY-MM-DD`, consistent with lease/payment dates.
                    .col(col("due_date").string().not_null())
                    // Days before the due date to notify, e.g. `[30, 7, 1]`.
                    .col(col("lead_days").json_binary().not_null().default("[]"))
                    // External recipient email addresses (staff holding
                    // `calendar:read` are always notified in-app/push).
                    .col(col("recipients").json_binary().not_null().default("[]"))
                    // Lead times that have already fired, e.g. `[30]`.
                    .col(col("fired").json_binary().not_null().default("[]"))
                    // active | done | cancelled
                    .col(col("status").string().not_null().default("active"))
                    .col(col("completed_at").timestamp_with_time_zone().null())
                    // `None` = the pipeline created it (lease renewal sync).
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "reminder", "tenant_id").await?;
        index(manager, "reminder", "due_date").await?;
        index(manager, "reminder", "subject_id").await?;
        enforce_rls(manager, "reminder").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP POLICY IF EXISTS reminder_tenant_isolation ON reminder;")
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("reminder"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
