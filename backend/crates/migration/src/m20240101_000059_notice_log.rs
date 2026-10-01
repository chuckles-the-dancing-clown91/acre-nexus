//! **Notice log**: every automatic notice (rent due, inspection tomorrow,
//! warranty ending…) claims a unique key before it is sent, so a job that runs
//! every few hours never sends the same notice twice.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS notice_log (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    key text NOT NULL,
    sent_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_notice_log_key ON notice_log (tenant_id, key);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        db.execute_unprepared(&format!(
            "ALTER TABLE notice_log ENABLE ROW LEVEL SECURITY; \
             ALTER TABLE notice_log FORCE ROW LEVEL SECURITY; \
             DROP POLICY IF EXISTS notice_log_tenant_isolation ON notice_log; \
             CREATE POLICY notice_log_tenant_isolation ON notice_log \
               USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS notice_log")
            .await?;
        Ok(())
    }
}
