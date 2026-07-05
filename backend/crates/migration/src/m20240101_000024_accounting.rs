//! **Payments + accounting core** (roadmap Phase 3, issues #33–#39).
//!
//! * `ledger_account` — the chart of accounts, partitioned per legal entity
//!   (`llc.id`): every LLC keeps its own books. Seeded system accounts carry a
//!   stable `subtype` the posting rules resolve by; `is_trust` marks the
//!   segregated escrow accounts that the no-commingling invariant guards.
//! * `ledger_txn` + `ledger_entry` — double-entry journal: a transaction is a
//!   balanced set of debit/credit entries (enforced in `api::accounting`, the
//!   single posting path). Entries carry optional property/lease dimensions
//!   for reporting.
//! * `payment_method` — tokenized saved payment instruments (Stripe payment
//!   method ids or simulated tokens — never PANs), with autopay enrollment.
//! * `bank_txn` — bank-feed transactions pulled per linked `bank_account`
//!   (Plaid or simulated), matched against settled payments during
//!   reconciliation.
//! * `owner_payout` — owner draws computed from the ledger per entity/period,
//!   executed as an ACH transfer with a generated statement document.
//! * `financial_snapshot` — monthly per-tenant rollups (occupancy,
//!   delinquency, portfolio value…) captured by the billing cycle so the
//!   dashboards can chart history that cannot be derived retroactively.
//! * `lease_payment` grows from a bare ledger line into a receivable +
//!   payment: a `kind`, the charging method/provider/external id, failure
//!   reason, receipt number, and the ledger transaction it posted.
//! * `bank_account` gains provider linkage (Plaid item/account) + last sync.
//!
//! Every new table is tenant-owned and gets the same enforced RLS as
//! `m20240101_000015_rls_enforce`.

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
        // ---- ledger_account (chart of accounts, per legal entity) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("ledger_account"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("entity_id").uuid().not_null())
                    .col(col("code").string().not_null())
                    .col(col("name").string().not_null())
                    // asset | liability | equity | income | expense
                    .col(col("kind").string().not_null())
                    // Stable hook posting rules resolve by (operating_bank,
                    // trust_bank, accounts_receivable, security_deposits, …).
                    .col(col("subtype").string().null())
                    .col(col("is_trust").boolean().not_null().default(false))
                    // Seeded default-chart accounts (vs custom additions).
                    .col(col("system").boolean().not_null().default(false))
                    .col(col("active").boolean().not_null().default(true))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "ledger_account", "tenant_id").await?;
        index(manager, "ledger_account", "entity_id").await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_ledger_account_entity_code \
                   ON ledger_account (tenant_id, entity_id, code);",
            )
            .await?;
        enforce_rls(manager, "ledger_account").await?;

        // ---- ledger_txn (journal transaction header) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("ledger_txn"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("entity_id").uuid().not_null())
                    // `YYYY-MM-DD`, consistent with lease/payment dates.
                    .col(col("txn_date").string().not_null())
                    .col(col("memo").string().not_null())
                    // rent_due | payment | deposit | late_fee | payout | manual …
                    .col(col("source_type").string().not_null())
                    .col(col("source_id").uuid().null())
                    .col(col("posted_by").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "ledger_txn", "tenant_id").await?;
        index(manager, "ledger_txn", "entity_id").await?;
        index(manager, "ledger_txn", "txn_date").await?;
        enforce_rls(manager, "ledger_txn").await?;

        // ---- ledger_entry (one debit or credit leg) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("ledger_entry"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("txn_id").uuid().not_null())
                    .col(col("account_id").uuid().not_null())
                    // `debit` | `credit`; amounts are always positive.
                    .col(col("side").string().not_null())
                    .col(col("amount_cents").big_integer().not_null())
                    // Optional reporting dimensions.
                    .col(col("property_id").uuid().null())
                    .col(col("lease_id").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "ledger_entry", "tenant_id").await?;
        index(manager, "ledger_entry", "txn_id").await?;
        index(manager, "ledger_entry", "account_id").await?;
        enforce_rls(manager, "ledger_entry").await?;

        // ---- payment_method (tokenized saved instruments; never PANs) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("payment_method"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().null())
                    .col(col("user_id").uuid().null())
                    // stripe | simulated
                    .col(col("provider").string().not_null())
                    // card | ach
                    .col(col("kind").string().not_null())
                    // Provider token (pm_… / btok_… / sim_…), never card data.
                    .col(col("external_id").string().not_null())
                    .col(col("brand").string().null())
                    .col(col("last4").string().not_null())
                    .col(col("exp_month").integer().null())
                    .col(col("exp_year").integer().null())
                    // active | removed
                    .col(col("status").string().not_null().default("active"))
                    .col(col("autopay").boolean().not_null().default(false))
                    // Day of month autopay charges (clamped 1–28).
                    .col(col("autopay_day").integer().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "payment_method", "tenant_id").await?;
        index(manager, "payment_method", "lease_id").await?;
        index(manager, "payment_method", "user_id").await?;
        // At most one active autopay enrollment per lease.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_payment_method_autopay \
                   ON payment_method (tenant_id, lease_id) \
                   WHERE autopay AND status = 'active';",
            )
            .await?;
        enforce_rls(manager, "payment_method").await?;

        // ---- bank_txn (bank-feed lines for reconciliation) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("bank_txn"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("bank_account_id").uuid().not_null())
                    .col(col("posted_date").string().not_null())
                    .col(col("description").string().not_null())
                    // Signed: positive = deposit into the account.
                    .col(col("amount_cents").big_integer().not_null())
                    // Provider transaction id (dedupes re-syncs).
                    .col(col("external_id").string().not_null())
                    // unmatched | matched | ignored
                    .col(col("status").string().not_null().default("unmatched"))
                    .col(col("matched_payment_id").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "bank_txn", "tenant_id").await?;
        index(manager, "bank_txn", "bank_account_id").await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_bank_txn_external \
                   ON bank_txn (bank_account_id, external_id);",
            )
            .await?;
        enforce_rls(manager, "bank_txn").await?;

        // ---- owner_payout (draws + statements) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("owner_payout"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("entity_id").uuid().not_null())
                    .col(col("period_start").string().not_null())
                    .col(col("period_end").string().not_null())
                    .col(col("rent_collected_cents").big_integer().not_null())
                    .col(col("expenses_cents").big_integer().not_null())
                    .col(col("mgmt_fee_cents").big_integer().not_null())
                    .col(col("net_cents").big_integer().not_null())
                    // draft | processing | paid | failed
                    .col(col("status").string().not_null().default("draft"))
                    .col(col("provider").string().null())
                    .col(col("external_id").string().null())
                    .col(col("statement_document_id").uuid().null())
                    .col(col("ledger_txn_id").uuid().null())
                    .col(col("failure_reason").text().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "owner_payout", "tenant_id").await?;
        index(manager, "owner_payout", "entity_id").await?;
        enforce_rls(manager, "owner_payout").await?;

        // ---- financial_snapshot (monthly dashboard history) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("financial_snapshot"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    // `YYYY-MM`.
                    .col(col("month").string().not_null())
                    .col(col("occupancy_bps").integer().not_null().default(0))
                    .col(col("delinquency_bps").integer().not_null().default(0))
                    .col(
                        col("portfolio_value_cents")
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(col("rent_due_cents").big_integer().not_null().default(0))
                    .col(
                        col("rent_collected_cents")
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(col("noi_cents").big_integer().not_null().default(0))
                    .col(col("active_leases").integer().not_null().default(0))
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "financial_snapshot", "tenant_id").await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_financial_snapshot_month \
                   ON financial_snapshot (tenant_id, month);",
            )
            .await?;
        enforce_rls(manager, "financial_snapshot").await?;

        // ---- lease_payment: receivable + payment lifecycle columns ----
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE lease_payment \
                   ADD COLUMN IF NOT EXISTS kind VARCHAR NOT NULL DEFAULT 'rent', \
                   ADD COLUMN IF NOT EXISTS method_id UUID NULL, \
                   ADD COLUMN IF NOT EXISTS provider VARCHAR NULL, \
                   ADD COLUMN IF NOT EXISTS external_id VARCHAR NULL, \
                   ADD COLUMN IF NOT EXISTS failure_reason TEXT NULL, \
                   ADD COLUMN IF NOT EXISTS receipt_number VARCHAR NULL, \
                   ADD COLUMN IF NOT EXISTS ledger_txn_id UUID NULL;",
            )
            .await?;
        index(manager, "lease_payment", "external_id").await?;

        // ---- bank_account: provider linkage for the bank feed ----
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE bank_account \
                   ADD COLUMN IF NOT EXISTS provider VARCHAR NULL, \
                   ADD COLUMN IF NOT EXISTS external_id VARCHAR NULL, \
                   ADD COLUMN IF NOT EXISTS last_synced_at TIMESTAMPTZ NULL;",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE bank_account \
               DROP COLUMN IF EXISTS provider, \
               DROP COLUMN IF EXISTS external_id, \
               DROP COLUMN IF EXISTS last_synced_at; \
             ALTER TABLE lease_payment \
               DROP COLUMN IF EXISTS kind, \
               DROP COLUMN IF EXISTS method_id, \
               DROP COLUMN IF EXISTS provider, \
               DROP COLUMN IF EXISTS external_id, \
               DROP COLUMN IF EXISTS failure_reason, \
               DROP COLUMN IF EXISTS receipt_number, \
               DROP COLUMN IF EXISTS ledger_txn_id;",
        )
        .await?;
        for table in [
            "financial_snapshot",
            "owner_payout",
            "bank_txn",
            "payment_method",
            "ledger_entry",
            "ledger_txn",
            "ledger_account",
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
