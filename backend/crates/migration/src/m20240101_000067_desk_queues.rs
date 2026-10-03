//! **Queues**: a task can be given to a person on the team (not only a
//! vendor), and remembers how it reached a vendor.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE ticket_task ADD COLUMN IF NOT EXISTS assignee_user_id uuid NULL;
ALTER TABLE ticket_task ADD COLUMN IF NOT EXISTS dispatch_via text NULL;
ALTER TABLE ticket_task ADD COLUMN IF NOT EXISTS dispatch_note text NULL;
CREATE INDEX IF NOT EXISTS idx_ticket_task_assignee ON ticket_task (tenant_id, assignee_user_id) WHERE assignee_user_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ticket_assignee ON maintenance_ticket (tenant_id, assignee_user_id) WHERE assignee_user_id IS NOT NULL;
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
                "DROP INDEX IF EXISTS idx_ticket_assignee; \
                 DROP INDEX IF EXISTS idx_ticket_task_assignee; \
                 ALTER TABLE ticket_task DROP COLUMN IF EXISTS dispatch_note; \
                 ALTER TABLE ticket_task DROP COLUMN IF EXISTS dispatch_via; \
                 ALTER TABLE ticket_task DROP COLUMN IF EXISTS assignee_user_id;",
            )
            .await?;
        Ok(())
    }
}
