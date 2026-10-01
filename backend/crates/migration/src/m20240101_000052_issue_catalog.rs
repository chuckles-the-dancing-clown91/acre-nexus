//! **Issue catalog** (roadmap area 4): the common problems a maintenance
//! employee picks from. Each carries a category, priority, a checklist, and
//! the parts it usually needs; "generate ticket" opens the work order and
//! builds the shopping list from inventory in one click.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS issue_template (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    name text NOT NULL,
    area text NULL,
    category text NOT NULL DEFAULT 'general',
    priority text NOT NULL DEFAULT 'normal',
    description text NULL,
    est_minutes integer NULL,
    checklist jsonb NOT NULL DEFAULT '[]'::jsonb,
    parts jsonb NOT NULL DEFAULT '[]'::jsonb,
    active boolean NOT NULL DEFAULT true,
    seeded boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_issue_template_tenant ON issue_template (tenant_id, active);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        db.execute_unprepared(&format!(
            "ALTER TABLE issue_template ENABLE ROW LEVEL SECURITY; \
             ALTER TABLE issue_template FORCE ROW LEVEL SECURITY; \
             DROP POLICY IF EXISTS issue_template_tenant_isolation ON issue_template; \
             CREATE POLICY issue_template_tenant_isolation ON issue_template \
               USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS issue_template")
            .await?;
        Ok(())
    }
}
