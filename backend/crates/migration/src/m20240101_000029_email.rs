//! **Email integration** (issue #62) — inbound email routing, CRM leads, and
//! white-label deliverability.
//!
//! * `lead` — a leasing prospect (the seed of the CRM, #46): created/updated
//!   when mail arrives at the tenant's monitored leasing inbox, and workable
//!   from the console (`new → contacted → toured → applied → closed`).
//! * `inbound_email` — the comms log: every inbound message with where it was
//!   routed (a ticket comment, a lead, or unmatched).
//! * `domain` grows email deliverability state: the per-record SPF/DKIM/DMARC
//!   check results and an `email_verified_at` timestamp once all three pass,
//!   mirroring the existing domain-verify flow.

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
        // ---- lead (leasing CRM prospect, the #46 seed) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("lead"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("email").string().not_null())
                    .col(col("phone").string().null())
                    // inbound_email | manual | website
                    .col(col("source").string().not_null().default("manual"))
                    // new | contacted | toured | applied | closed
                    .col(col("status").string().not_null().default("new"))
                    .col(col("notes").text().null())
                    // The latest inbound message (subject + excerpt).
                    .col(col("last_message").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "lead", "tenant_id").await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_lead_email \
                   ON lead (tenant_id, email);",
            )
            .await?;
        enforce_rls(manager, "lead").await?;

        // ---- inbound_email (the comms log) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("inbound_email"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("from_email").string().not_null())
                    .col(col("to_email").string().not_null())
                    .col(col("subject").string().not_null())
                    .col(col("body_text").text().not_null())
                    // ticket_comment | lead | unmatched
                    .col(col("routed").string().not_null().default("unmatched"))
                    // The ticket-comment or lead row the message landed on.
                    .col(col("routed_id").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "inbound_email", "tenant_id").await?;
        enforce_rls(manager, "inbound_email").await?;

        // ---- domain: email deliverability state ----
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE domain \
                   ADD COLUMN IF NOT EXISTS email_dns_status JSONB NOT NULL DEFAULT '{}', \
                   ADD COLUMN IF NOT EXISTS email_verified_at TIMESTAMPTZ NULL;",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE domain \
               DROP COLUMN IF EXISTS email_dns_status, \
               DROP COLUMN IF EXISTS email_verified_at;",
        )
        .await?;
        for table in ["inbound_email", "lead"] {
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
