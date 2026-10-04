//! **Work order feed and time tracking.** A note can belong to one task
//! (so a task shows its own notes and vendor updates), and each work order
//! can have time tracking on or off for the in-house crew.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const SQL: &str = r#"
ALTER TABLE ticket_comment
    ADD COLUMN IF NOT EXISTS task_id uuid NULL REFERENCES ticket_task(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS ix_ticket_comment_task ON ticket_comment (task_id);
ALTER TABLE maintenance_ticket
    ADD COLUMN IF NOT EXISTS track_time boolean NOT NULL DEFAULT true;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(SQL).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE maintenance_ticket DROP COLUMN IF EXISTS track_time; \
                 ALTER TABLE ticket_comment DROP COLUMN IF EXISTS task_id;",
            )
            .await?;
        Ok(())
    }
}
