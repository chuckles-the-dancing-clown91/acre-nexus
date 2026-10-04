//! **Texts, round 2**: a conversation from a number that isn't a resident can
//! be linked to a prospect (lead) or a vendor, and a missed call is filed in
//! the conversation it came from.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE sms_thread
    ADD COLUMN IF NOT EXISTS lead_id uuid NULL,
    ADD COLUMN IF NOT EXISTS counterparty_id uuid NULL;
CREATE INDEX IF NOT EXISTS ix_sms_thread_lead ON sms_thread (lead_id) WHERE lead_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS ix_sms_thread_counterparty ON sms_thread (counterparty_id) WHERE counterparty_id IS NOT NULL;
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
                "DROP INDEX IF EXISTS ix_sms_thread_counterparty; \
                 DROP INDEX IF EXISTS ix_sms_thread_lead; \
                 ALTER TABLE sms_thread DROP COLUMN IF EXISTS counterparty_id, DROP COLUMN IF EXISTS lead_id;",
            )
            .await?;
        Ok(())
    }
}
