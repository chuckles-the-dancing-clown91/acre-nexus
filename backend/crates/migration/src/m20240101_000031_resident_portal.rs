//! **Tenant lifecycle & resident portal round-out** (Phase 5, issue #9).
//!
//! * `message_thread` / `message` — resident ↔ manager messaging: one thread
//!   per conversation on a lease, with a flat message timeline underneath.
//! * `inspection` / `inspection_item` — move-in / move-out inspections: a
//!   checklist of condition items per lease event, photos ride the document
//!   service (`owner_type = "inspection"`).
//! * `deposit_disposition` / `deposit_deduction` — the security-deposit
//!   settlement at move-out: itemized deductions, the refund executed through
//!   the payments provider, and the generated statement filed on the lease.
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
                    .table(Alias::new("message_thread"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("subject").string().not_null())
                    // open | closed
                    .col(col("status").string().not_null().default("open"))
                    .col(col("created_by").uuid().not_null())
                    .col(ts("last_message_at"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "message_thread", "tenant_id").await?;
        index(manager, "message_thread", "lease_id").await?;
        enforce_rls(manager, "message_thread").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("message"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("thread_id").uuid().not_null())
                    .col(col("sender_user_id").uuid().not_null())
                    // resident | staff
                    .col(col("sender_kind").string().not_null())
                    .col(col("sender_name").string().not_null())
                    .col(col("body").text().not_null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "message", "tenant_id").await?;
        index(manager, "message", "thread_id").await?;
        enforce_rls(manager, "message").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("inspection"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("unit_id").uuid().null())
                    // move_in | move_out
                    .col(col("kind").string().not_null())
                    // draft | completed
                    .col(col("status").string().not_null().default("draft"))
                    // ISO date, like `lease.start_date` / `maintenance_ticket.due_date`.
                    .col(col("scheduled_date").string().null())
                    .col(col("completed_at").timestamp_with_time_zone().null())
                    .col(col("completed_by").uuid().null())
                    .col(col("notes").text().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "inspection", "tenant_id").await?;
        index(manager, "inspection", "lease_id").await?;
        enforce_rls(manager, "inspection").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("inspection_item"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("inspection_id").uuid().not_null())
                    .col(col("area").string().not_null())
                    .col(col("item").string().not_null())
                    // unrated | good | fair | poor | damaged
                    .col(col("condition").string().not_null().default("unrated"))
                    .col(col("notes").text().null())
                    .col(col("sort_order").integer().not_null().default(0))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "inspection_item", "tenant_id").await?;
        index(manager, "inspection_item", "inspection_id").await?;
        enforce_rls(manager, "inspection_item").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("deposit_disposition"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    // draft | processing | closed | failed
                    .col(col("status").string().not_null().default("draft"))
                    .col(col("deposit_cents").big_integer().not_null())
                    .col(col("refund_cents").big_integer().null())
                    .col(col("notes").text().null())
                    // Refund execution (mirrors `payout`): provider + external id.
                    .col(col("provider").string().null())
                    .col(col("external_id").string().null())
                    .col(col("failure_reason").text().null())
                    // The generated disposition-statement PDF in the document service.
                    .col(col("statement_document_id").uuid().null())
                    .col(col("finalized_by").uuid().null())
                    .col(col("finalized_at").timestamp_with_time_zone().null())
                    .col(col("closed_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "deposit_disposition", "tenant_id").await?;
        index(manager, "deposit_disposition", "lease_id").await?;
        enforce_rls(manager, "deposit_disposition").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("deposit_deduction"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("disposition_id").uuid().not_null())
                    .col(col("description").string().not_null())
                    .col(col("amount_cents").big_integer().not_null())
                    .col(col("sort_order").integer().not_null().default(0))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "deposit_deduction", "tenant_id").await?;
        index(manager, "deposit_deduction", "disposition_id").await?;
        enforce_rls(manager, "deposit_deduction").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in [
            "deposit_deduction",
            "deposit_disposition",
            "inspection_item",
            "inspection",
            "message",
            "message_thread",
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
