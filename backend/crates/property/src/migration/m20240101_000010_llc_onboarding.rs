//! LLC onboarding schema (property database):
//! * **alter `llc`** — add the onboarding profile columns (entity type, filing
//!   + contact details, lifecycle `status`, timestamps).
//! * **`llc_document`** — uploaded files (logo, formation docs, EIN letter, …);
//!   bytes live in the object store, this row is metadata + `storage_key`.
//! * **`llc_branding`** — one-per-LLC logo / colours / signature / verbiage.
//! * **`llc_template`** — reusable lease / letter / email Handlebars templates.
//! * **`generated_document`** — rendered lease/letter PDFs.
//!
//! All four new tables are tenant-scoped, so they get the same row-level-security
//! policy used by the rest of the property database (`llc` already has one).

use super::{col, index, ts, uuid_pk};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// New tenant-scoped tables introduced here that need an RLS policy.
const NEW_TENANT_SCOPED: &[&str] = &[
    "llc_document",
    "llc_branding",
    "llc_template",
    "generated_document",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- extend llc with the onboarding profile ----
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("llc"))
                    .add_column(col("entity_type").string().not_null().default("LLC"))
                    .add_column(col("formation_date").string().null())
                    .add_column(col("registered_agent").string().null())
                    .add_column(col("principal_address").string().null())
                    .add_column(col("mailing_address").string().null())
                    .add_column(col("contact_name").string().null())
                    .add_column(col("contact_email").string().null())
                    .add_column(col("contact_phone").string().null())
                    .add_column(col("website").string().null())
                    .add_column(col("status").string().not_null().default("draft"))
                    .add_column(col("onboarded_at").timestamp_with_time_zone().null())
                    .add_column(ts("updated_at"))
                    .to_owned(),
            )
            .await?;

        // ---- llc_document ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("llc_document"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("llc_id").uuid().not_null())
                    .col(col("kind").string().not_null().default("other"))
                    .col(col("title").string().null())
                    .col(col("original_filename").string().not_null().default(""))
                    .col(col("mime_type").string().not_null().default("application/octet-stream"))
                    .col(col("size_bytes").big_integer().not_null().default(0))
                    .col(col("storage_provider").string().not_null().default("platform"))
                    .col(col("storage_key").string().not_null().default(""))
                    .col(col("sha256").string().not_null().default(""))
                    .col(col("uploaded_by").uuid().null())
                    .col(col("verified_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "llc_document", "llc_id").await?;

        // ---- llc_branding (one row per llc) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("llc_branding"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("llc_id").uuid().not_null().unique_key())
                    .col(col("logo_document_id").uuid().null())
                    .col(col("primary_color").string().null())
                    .col(col("accent_color").string().null())
                    .col(col("signature_name").string().null())
                    .col(col("signature_title").string().null())
                    .col(col("signature_block").text().null())
                    .col(col("letterhead").text().null())
                    .col(col("footer").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;

        // ---- llc_template ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("llc_template"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("llc_id").uuid().not_null())
                    .col(col("kind").string().not_null().default("other"))
                    .col(col("name").string().not_null().default(""))
                    .col(col("subject").string().null())
                    .col(col("body").text().not_null().default(""))
                    .col(col("is_default").boolean().not_null().default(false))
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "llc_template", "llc_id").await?;

        // ---- generated_document ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("generated_document"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("llc_id").uuid().not_null())
                    .col(col("template_id").uuid().null())
                    .col(col("lease_id").uuid().null())
                    .col(col("kind").string().not_null().default("letter"))
                    .col(col("title").string().not_null().default(""))
                    .col(col("storage_provider").string().not_null().default("platform"))
                    .col(col("storage_key").string().not_null().default(""))
                    .col(col("mime_type").string().not_null().default("application/pdf"))
                    .col(col("size_bytes").big_integer().not_null().default(0))
                    .col(col("status").string().not_null().default("draft"))
                    .col(col("rendered_by").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "generated_document", "llc_id").await?;
        index(manager, "generated_document", "lease_id").await?;

        // ---- row-level security for the new tenant-scoped tables ----
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
        for c in [
            "entity_type",
            "formation_date",
            "registered_agent",
            "principal_address",
            "mailing_address",
            "contact_name",
            "contact_email",
            "contact_phone",
            "website",
            "status",
            "onboarded_at",
            "updated_at",
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("llc"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
