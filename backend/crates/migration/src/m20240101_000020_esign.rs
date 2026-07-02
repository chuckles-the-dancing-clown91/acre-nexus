//! **E-signature envelopes** (roadmap Phase 2): the native envelope flow that
//! sends a generated lease document out for signature.
//!
//! * `esign_envelope` — one lease document sent for signature; tracks the
//!   collective state (`sent` → `partially_signed` → `completed`, or
//!   `declined` / `voided`) and pins a SHA-256 of the body at send time.
//! * `esign_signer` — one party on the envelope (resident / landlord /
//!   guarantor / other). Stores only the SHA-256 hash of the tokenized signing
//!   link plus the party's signature record (typed name, timestamp, IP, UA).
//! * `esign_event` — the append-only ESIGN/UETA audit trail (sent, viewed,
//!   signed, declined, reminded, completed, voided) with IP + user agent.
//!
//! All three tables are tenant-owned with the same enforced RLS as every other
//! tenant table.

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
        // ---- esign_envelope ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("esign_envelope"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("lease_id").uuid().not_null())
                    .col(col("lease_document_id").uuid().not_null())
                    .col(col("title").string().not_null())
                    .col(col("message").text().null())
                    .col(col("status").string().not_null())
                    .col(col("body_hash").string().not_null())
                    .col(col("signed_document_id").uuid().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("sent_at"))
                    .col(col("completed_at").timestamp_with_time_zone().null())
                    .col(col("voided_at").timestamp_with_time_zone().null())
                    .col(col("void_reason").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "esign_envelope", "tenant_id").await?;
        index(manager, "esign_envelope", "lease_id").await?;
        index(manager, "esign_envelope", "lease_document_id").await?;
        enforce_rls(manager, "esign_envelope").await?;

        // ---- esign_signer ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("esign_signer"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("envelope_id").uuid().not_null())
                    .col(col("role").string().not_null())
                    .col(col("name").string().not_null())
                    .col(col("email").string().not_null())
                    .col(col("phone").string().null())
                    // The raw token never persists — only its SHA-256.
                    .col(col("token_hash").string().not_null().unique_key())
                    .col(col("status").string().not_null())
                    .col(col("viewed_at").timestamp_with_time_zone().null())
                    .col(col("signed_at").timestamp_with_time_zone().null())
                    .col(col("signed_name").string().null())
                    .col(col("signed_ip").string().null())
                    .col(col("signed_user_agent").text().null())
                    .col(col("decline_reason").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "esign_signer", "tenant_id").await?;
        index(manager, "esign_signer", "envelope_id").await?;
        enforce_rls(manager, "esign_signer").await?;

        // ---- esign_event (append-only audit trail) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("esign_event"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("envelope_id").uuid().not_null())
                    .col(col("signer_id").uuid().null())
                    .col(col("event").string().not_null())
                    .col(col("detail").json_binary().not_null().default("{}"))
                    .col(col("ip").string().null())
                    .col(col("user_agent").text().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "esign_event", "tenant_id").await?;
        index(manager, "esign_event", "envelope_id").await?;
        enforce_rls(manager, "esign_event").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "DROP POLICY IF EXISTS esign_event_tenant_isolation ON esign_event; \
             DROP POLICY IF EXISTS esign_signer_tenant_isolation ON esign_signer; \
             DROP POLICY IF EXISTS esign_envelope_tenant_isolation ON esign_envelope;",
        )
        .await?;
        for table in ["esign_event", "esign_signer", "esign_envelope"] {
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
