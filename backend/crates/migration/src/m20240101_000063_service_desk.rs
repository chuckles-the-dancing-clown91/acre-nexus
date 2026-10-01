//! **Service desk**: job kits with task line items, the tasks on a work order
//! (each with its trade and whether it needs a contractor), photos attached to
//! notes, the trades a vendor covers, and kits on routine maintenance.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE issue_template ADD COLUMN IF NOT EXISTS tasks jsonb NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE ticket_comment ADD COLUMN IF NOT EXISTS document_ids jsonb NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE counterparty ADD COLUMN IF NOT EXISTS trades jsonb NOT NULL DEFAULT '[]'::jsonb;
ALTER TABLE maintenance_plan ADD COLUMN IF NOT EXISTS issue_template_id uuid NULL;

CREATE TABLE IF NOT EXISTS ticket_task (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    ticket_id uuid NOT NULL,
    position integer NOT NULL DEFAULT 0,
    title text NOT NULL,
    trade text NOT NULL DEFAULT 'general',
    est_minutes integer NULL,
    est_cost_cents bigint NULL,
    needs_contractor boolean NOT NULL DEFAULT false,
    assignee_entity_id uuid NULL,
    status text NOT NULL DEFAULT 'todo',
    done_at timestamptz NULL,
    done_by uuid NULL,
    dispatched_at timestamptz NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_ticket_task_ticket ON ticket_task (tenant_id, ticket_id, position);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        let t = "ticket_task";
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
            .execute_unprepared(
                "DROP TABLE IF EXISTS ticket_task; \
                 ALTER TABLE maintenance_plan DROP COLUMN IF EXISTS issue_template_id; \
                 ALTER TABLE counterparty DROP COLUMN IF EXISTS trades; \
                 ALTER TABLE ticket_comment DROP COLUMN IF EXISTS document_ids; \
                 ALTER TABLE issue_template DROP COLUMN IF EXISTS tasks;",
            )
            .await?;
        Ok(())
    }
}
