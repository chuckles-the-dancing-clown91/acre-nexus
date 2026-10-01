//! **Listing photos** (fix plan F18): ordered, captioned pictures on a listing,
//! each with the alt text screen readers and search engines read. The first is
//! the hero, the share image and the structured-data image.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS listing_photo (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    listing_id uuid NOT NULL,
    document_id uuid NOT NULL,
    alt_text text NOT NULL,
    caption text NULL,
    position integer NOT NULL DEFAULT 0,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_listing_photo_listing ON listing_photo (tenant_id, listing_id, position);
CREATE UNIQUE INDEX IF NOT EXISTS uq_listing_photo_document ON listing_photo (document_id);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        let t = "listing_photo";
        let policy = format!("{t}_tenant_isolation");
        db.execute_unprepared(&format!(
            "ALTER TABLE {t} ENABLE ROW LEVEL SECURITY; \
             ALTER TABLE {t} FORCE ROW LEVEL SECURITY; \
             DROP POLICY IF EXISTS {policy} ON {t}; \
             CREATE POLICY {policy} ON {t} \
               USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS listing_photo")
            .await?;
        Ok(())
    }
}
