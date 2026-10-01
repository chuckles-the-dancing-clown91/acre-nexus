//! **Single sign-on with Alpha** (Vantedge ↔ Alpha): a table of used
//! assertion ids so each signed sign-in token works once, and the vendor's
//! Alpha web address for the "Open in Alpha" launch.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS sso_assertion (
    jti text PRIMARY KEY,
    tenant_id uuid NULL,
    issuer text NOT NULL,
    subject text NOT NULL,
    used_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_sso_assertion_used ON sso_assertion (used_at);
ALTER TABLE counterparty ADD COLUMN IF NOT EXISTS partner_web_url text NULL;
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
            .execute_unprepared(
                "DROP TABLE IF EXISTS sso_assertion; \
                 ALTER TABLE counterparty DROP COLUMN IF EXISTS partner_web_url;",
            )
            .await?;
        Ok(())
    }
}
