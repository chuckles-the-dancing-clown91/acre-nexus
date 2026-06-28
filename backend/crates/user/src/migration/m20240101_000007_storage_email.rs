//! Storage + email schema (user database):
//! * **`tenant_storage_config`** — per-tenant object-storage backend (platform-
//!   managed by default, or a BYO `local` / `s3` / `gcs` bucket). The credential
//!   blob is sealed (AES-256-GCM); only non-secret settings are stored in clear.
//! * **`sent_email`** — durable log of emails the platform sent or simulated.
//!
//! Both are tenant-scoped, so they receive the standard row-level-security policy.

use super::{col, index, ts, uuid_pk};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const NEW_TENANT_SCOPED: &[&str] = &["tenant_storage_config", "sent_email"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- tenant_storage_config (one row per tenant) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("tenant_storage_config"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null().unique_key())
                    .col(col("provider").string().not_null().default("platform"))
                    .col(col("bucket").string().null())
                    .col(col("region").string().null())
                    .col(col("prefix").string().null())
                    .col(col("endpoint").string().null())
                    .col(col("secret_ciphertext").text().null())
                    .col(col("secret_nonce").string().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;

        // ---- sent_email ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("sent_email"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("llc_id").uuid().null())
                    .col(col("to_address").string().not_null().default(""))
                    .col(col("cc").string().null())
                    .col(col("subject").string().not_null().default(""))
                    .col(col("body").text().not_null().default(""))
                    .col(col("template_id").uuid().null())
                    .col(col("provider").string().not_null().default("log"))
                    .col(col("status").string().not_null().default("simulated"))
                    .col(col("error").text().null())
                    .col(col("job_id").uuid().null())
                    .col(col("generated_document_id").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "sent_email", "tenant_id").await?;

        // ---- row-level security ----
        let db = manager.get_connection();
        for t in NEW_TENANT_SCOPED {
            let sql = format!(
                "ALTER TABLE {t} ENABLE ROW LEVEL SECURITY; \
                 ALTER TABLE {t} FORCE ROW LEVEL SECURITY; \
                 DROP POLICY IF EXISTS {t}_tenant_isolation ON {t}; \
                 CREATE POLICY {t}_tenant_isolation ON {t} \
                   USING (\
                     current_setting('app.tenant_id', true) IS NULL \
                     OR tenant_id::text = current_setting('app.tenant_id', true)\
                   );"
            );
            db.execute_unprepared(&sql).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in NEW_TENANT_SCOPED {
            manager
                .drop_table(Table::drop().table(Alias::new(*t)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
