//! **Leasing CRM & lease renewals** (issue #44) — closes the last gap in
//! `FEATURES.md` §2 ("Leasing, marketing & CRM").
//!
//! * `lease_renewal` — a proposed change of terms on an existing tenancy
//!   (typically a rent increase + extended end date). It rides the Phase 2
//!   document/e-sign substrate: propose → generate an addendum
//!   (`lease_document`) → send it out as an `esign_envelope` → on completion
//!   the new terms are applied to the underlying `lease`.
//! * `esign_envelope.purpose` — distinguishes an initial lease signing
//!   (`lease`, the default) from a renewal addendum (`renewal`), so envelope
//!   completion applies the right side-effects (activate a new tenancy vs.
//!   bump an existing one's rent + term).
//! * `lead.application_id` — the CRM lead → application link, set when a
//!   prospect is converted without leaving the platform.
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
                    .table(Alias::new("lease_renewal"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().not_null())
                    // proposed | sent | signed | activated | declined | cancelled
                    .col(col("status").string().not_null().default("proposed"))
                    // The terms this renewal moves *from* (pinned at propose time).
                    .col(col("current_rent_cents").big_integer().not_null())
                    // The terms it moves *to*.
                    .col(col("new_rent_cents").big_integer().not_null())
                    // `YYYY-MM-DD` — effective date of the renewed term.
                    .col(col("new_start_date").string().not_null())
                    // `YYYY-MM-DD`, or NULL for month-to-month.
                    .col(col("new_end_date").string().null())
                    .col(col("term_months").integer().null())
                    .col(col("notes").text().null())
                    // The generated addendum document + the envelope it's sent in.
                    .col(col("lease_document_id").uuid().null())
                    .col(col("envelope_id").uuid().null())
                    .col(col("created_by").uuid().null())
                    .col(col("activated_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "lease_renewal", "tenant_id").await?;
        index(manager, "lease_renewal", "lease_id").await?;
        index(manager, "lease_renewal", "lease_document_id").await?;
        enforce_rls(manager, "lease_renewal").await?;

        // `purpose` distinguishes an initial lease signing from a renewal
        // addendum — on the document (so the "latest lease agreement" lookups
        // skip addenda) and on the envelope (so completion applies the right
        // side-effects). The CRM lead → application link is set at conversion.
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE lease_document \
               ADD COLUMN IF NOT EXISTS purpose varchar NOT NULL DEFAULT 'lease'; \
             ALTER TABLE esign_envelope \
               ADD COLUMN IF NOT EXISTS purpose varchar NOT NULL DEFAULT 'lease'; \
             ALTER TABLE lead \
               ADD COLUMN IF NOT EXISTS application_id uuid NULL;",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "ALTER TABLE lead DROP COLUMN IF EXISTS application_id; \
             ALTER TABLE esign_envelope DROP COLUMN IF EXISTS purpose; \
             ALTER TABLE lease_document DROP COLUMN IF EXISTS purpose;",
        )
        .await?;
        db.execute_unprepared(
            "DROP POLICY IF EXISTS lease_renewal_tenant_isolation ON lease_renewal;",
        )
        .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("lease_renewal"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
