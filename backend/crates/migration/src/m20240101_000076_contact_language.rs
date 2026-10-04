//! **Spanish for residents**: the language each person reads, keyed by the
//! address we reach them at (an email, lowercased, or a phone in E.164), so a
//! message picks its language from who it's going to, whether that's a
//! resident, an applicant, a prospect or a vendor.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS contact_language (
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    contact text NOT NULL,
    language text NOT NULL CHECK (language IN ('en', 'es')),
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, contact)
);
ALTER TABLE contact_language ENABLE ROW LEVEL SECURITY;
ALTER TABLE contact_language FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS contact_language_tenant_isolation ON contact_language;
CREATE POLICY contact_language_tenant_isolation ON contact_language
    USING (NULLIF(current_setting('app.tenant_id', true), '') IS NULL
           OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (NULLIF(current_setting('app.tenant_id', true), '') IS NULL
           OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''));
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS contact_language;")
            .await?;
        Ok(())
    }
}
