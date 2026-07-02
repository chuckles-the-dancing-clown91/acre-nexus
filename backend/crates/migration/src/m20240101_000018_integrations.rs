//! **Integration substrate** (roadmap Phase 1, issues #15–#19).
//!
//! * `secret` — encrypted per-tenant + platform credential storage (AES-256-GCM
//!   under the dedicated `SECRETS_ENC_KEY`). `tenant_id` is nullable: `NULL`
//!   rows are platform-wide defaults readable from any tenant context, so the
//!   RLS policy is hand-written below instead of using the shared helper.
//! * `document` — polymorphic file metadata (owner = property/lease/application/
//!   entity/deal/…), versioned via `previous_version_id`, blob in the object
//!   store under `storage_key`.
//! * `notification` — outbound email/SMS history with delivery status and an
//!   idempotency key so retried jobs don't double-send.
//! * `theme.notification_templates` — tenant template overrides, sibling to
//!   `legal_templates`.
//!
//! `document` and `notification` are tenant-owned and get the same enforced RLS
//! as every other tenant table, matching `m20240101_000015_rls_enforce`.

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
        // ---- secret (encrypted integration credentials) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("secret"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    // NULL = platform-wide secret; tenant rows shadow it.
                    .col(col("tenant_id").uuid().null())
                    .col(col("key").string().not_null())
                    .col(col("ciphertext").text().not_null())
                    .col(col("nonce").string().not_null())
                    .col(col("last4").string().not_null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(col("rotated_at").timestamp_with_time_zone().null())
                    .to_owned(),
            )
            .await?;
        index(manager, "secret", "tenant_id").await?;
        // One value per (tenant, key); Postgres treats NULLs as distinct, so the
        // platform plane needs its own uniqueness guard on key alone.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_secret_tenant_key \
                   ON secret (tenant_id, key) WHERE tenant_id IS NOT NULL; \
                 CREATE UNIQUE INDEX IF NOT EXISTS uq_secret_platform_key \
                   ON secret (key) WHERE tenant_id IS NULL;",
            )
            .await?;
        // Custom RLS: a tenant context may READ platform-wide (NULL-tenant)
        // secrets — they are the fallback a provider client resolves — but may
        // only WRITE rows belonging to that tenant.
        manager
            .get_connection()
            .execute_unprepared(&format!(
                "ALTER TABLE secret ENABLE ROW LEVEL SECURITY; \
                 ALTER TABLE secret FORCE ROW LEVEL SECURITY; \
                 DROP POLICY IF EXISTS secret_tenant_isolation ON secret; \
                 CREATE POLICY secret_tenant_isolation ON secret \
                   USING (current_setting('app.tenant_id', true) IS NULL \
                          OR tenant_id IS NULL \
                          OR tenant_id::text = current_setting('app.tenant_id', true)) \
                   WITH CHECK ({RLS_PRED});"
            ))
            .await?;

        // ---- document (object-store file metadata) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("document"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("owner_type").string().not_null())
                    .col(col("owner_id").uuid().not_null())
                    .col(col("filename").string().not_null())
                    .col(col("mime_type").string().not_null())
                    .col(col("size_bytes").big_integer().not_null().default(0))
                    .col(col("checksum").string().null())
                    .col(col("version").integer().not_null().default(1))
                    .col(col("previous_version_id").uuid().null())
                    .col(col("storage_key").string().not_null())
                    .col(col("status").string().not_null().default("pending_upload"))
                    .col(
                        col("retention_expires_at")
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "document", "tenant_id").await?;
        index(manager, "document", "owner_id").await?;
        index(manager, "document", "storage_key").await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_document_owner")
                    .table(Alias::new("document"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("owner_type"))
                    .col(Alias::new("owner_id"))
                    .to_owned(),
            )
            .await?;
        enforce_rls(manager, "document").await?;

        // ---- notification (outbound email/SMS history) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("notification"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("channel").string().not_null())
                    .col(col("template_key").string().not_null())
                    .col(col("recipient").string().not_null())
                    .col(col("status").string().not_null().default("queued"))
                    .col(col("provider_message_id").string().null())
                    .col(col("subject").text().null())
                    .col(col("body").text().null())
                    .col(col("background_job_id").uuid().null())
                    .col(col("idempotency_key").string().null())
                    .col(col("last_error").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "notification", "tenant_id").await?;
        index(manager, "notification", "background_job_id").await?;
        // A given natural trigger sends at most once per tenant.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_notification_idempotency \
                   ON notification (tenant_id, idempotency_key) \
                   WHERE idempotency_key IS NOT NULL;",
            )
            .await?;
        enforce_rls(manager, "notification").await?;

        // ---- theme.notification_templates (tenant template overrides) ----
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE theme ADD COLUMN IF NOT EXISTS \
                   notification_templates JSONB NOT NULL DEFAULT '{}'::jsonb;",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "DROP POLICY IF EXISTS notification_tenant_isolation ON notification; \
             DROP POLICY IF EXISTS document_tenant_isolation ON document; \
             DROP POLICY IF EXISTS secret_tenant_isolation ON secret; \
             ALTER TABLE theme DROP COLUMN IF EXISTS notification_templates;",
        )
        .await?;
        for table in ["notification", "document", "secret"] {
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
