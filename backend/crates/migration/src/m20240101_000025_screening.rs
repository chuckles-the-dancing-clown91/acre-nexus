//! **Real tenant screening** (roadmap Phase 4, epic #8).
//!
//! * `screening_report` — one FCRA screening per application: the provider
//!   order (external id, packages), applicant consent stamp, the results
//!   (credit score, criminal/eviction record counts, the provider's
//!   recommendation), and the final policy verdict with its reasons. The
//!   summary lives here; anything bulkier a live provider returns belongs in
//!   the document service, not the row.
//! * `application` gains the FCRA lifecycle columns: when the applicant
//!   consented to screening, and when (and with what notice document) an
//!   adverse-action notice was sent.
//!
//! Tenant-owned, so RLS is enforced exactly as `m20240101_000015_rls_enforce`.

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
        // ---- screening_report ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("screening_report"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("application_id").uuid().not_null())
                    // Provider key: `checkr` (simulated unless LIVE_PROVIDERS).
                    .col(col("provider").string().not_null())
                    // Provider report id once ordered.
                    .col(col("external_id").string().null())
                    // pending | in_progress | complete | failed
                    .col(col("status").string().not_null().default("pending"))
                    // Ordered packages.
                    .col(col("include_credit").boolean().not_null().default(true))
                    .col(col("include_criminal").boolean().not_null().default(true))
                    .col(col("include_eviction").boolean().not_null().default(true))
                    // Consent stamp copied from the application at order time.
                    .col(col("consent_at").timestamp_with_time_zone().null())
                    // ---- results ----
                    .col(col("credit_score").integer().null())
                    .col(col("criminal_records").integer().null())
                    .col(col("eviction_records").integer().null())
                    // Provider recommendation: clear | consider.
                    .col(col("recommendation").string().null())
                    // Final policy verdict: cleared | failed (null until landed).
                    .col(col("result").string().null())
                    // Why (policy trips + record findings) — JSON array of strings.
                    .col(col("reasons").json_binary().null())
                    .col(col("completed_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "screening_report", "tenant_id").await?;
        index(manager, "screening_report", "application_id").await?;
        index(manager, "screening_report", "external_id").await?;
        // One screening per application (retries update the same report).
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_screening_report_application \
                   ON screening_report (tenant_id, application_id);",
            )
            .await?;
        enforce_rls(manager, "screening_report").await?;

        // ---- application: consent + adverse action ----
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE application \
                   ADD COLUMN IF NOT EXISTS screening_consent_at TIMESTAMPTZ NULL, \
                   ADD COLUMN IF NOT EXISTS adverse_action_at TIMESTAMPTZ NULL, \
                   ADD COLUMN IF NOT EXISTS adverse_action_document_id UUID NULL;",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE application \
               DROP COLUMN IF EXISTS screening_consent_at, \
               DROP COLUMN IF EXISTS adverse_action_at, \
               DROP COLUMN IF EXISTS adverse_action_document_id; \
             DROP POLICY IF EXISTS screening_report_tenant_isolation ON screening_report;",
        )
        .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("screening_report"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
