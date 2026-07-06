//! **Accounts payable** (issue #58) — vendor bills → approval → pay.
//!
//! * `vendor_bill` — a bill from a vendor (`counterparty`), optionally raised
//!   from a completed `maintenance_ticket`, carrying line items and a status
//!   lifecycle (`draft → submitted → approved → processing → paid`, with
//!   `failed` retryable and `void` terminal). Approval accrues the expense to
//!   the entity's ledger (`Dr Property Expenses / Cr Accounts Payable`);
//!   payment executes through the payments provider and clears the liability
//!   (`Dr Accounts Payable / Cr Operating Bank`).
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
                    .table(Alias::new("vendor_bill"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    // FK to `llc.id` — which entity's books the expense hits.
                    .col(col("entity_id").uuid().not_null())
                    // FK to `counterparty.id` — the vendor being paid.
                    .col(col("counterparty_id").uuid().not_null())
                    // Optional reporting dimension / origin links.
                    .col(col("property_id").uuid().null())
                    .col(col("maintenance_ticket_id").uuid().null())
                    // Human reference (`BILL-…`), unique per tenant.
                    .col(col("bill_number").string().not_null())
                    .col(col("memo").string().not_null())
                    // `[{ "description": …, "amount_cents": … }]`
                    .col(col("line_items").json_binary().not_null().default("[]"))
                    .col(col("amount_cents").big_integer().not_null())
                    // `YYYY-MM-DD`, consistent with lease/payment dates.
                    .col(col("due_date").string().null())
                    // draft | submitted | approved | processing | paid | failed | void
                    .col(col("status").string().not_null().default("draft"))
                    .col(col("submitted_by").uuid().null())
                    .col(col("submitted_at").timestamp_with_time_zone().null())
                    .col(col("approved_by").uuid().null())
                    .col(col("approved_at").timestamp_with_time_zone().null())
                    // Why the last reviewer sent it back to draft.
                    .col(col("rejected_reason").text().null())
                    // stripe | simulated (set when payment executes).
                    .col(col("provider").string().null())
                    .col(col("external_id").string().null())
                    // Approval accrual posting (Dr Expenses / Cr AP).
                    .col(col("accrual_txn_id").uuid().null())
                    // Payment posting (Dr AP / Cr Operating Bank).
                    .col(col("payment_txn_id").uuid().null())
                    .col(col("failure_reason").text().null())
                    .col(col("paid_at").timestamp_with_time_zone().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "vendor_bill", "tenant_id").await?;
        index(manager, "vendor_bill", "entity_id").await?;
        index(manager, "vendor_bill", "counterparty_id").await?;
        index(manager, "vendor_bill", "maintenance_ticket_id").await?;
        index(manager, "vendor_bill", "external_id").await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_vendor_bill_number \
                   ON vendor_bill (tenant_id, bill_number);",
            )
            .await?;
        enforce_rls(manager, "vendor_bill").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DROP POLICY IF EXISTS vendor_bill_tenant_isolation ON vendor_bill;",
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("vendor_bill"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
