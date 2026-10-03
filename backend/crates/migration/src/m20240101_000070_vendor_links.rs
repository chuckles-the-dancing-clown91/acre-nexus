//! **Vendor links**: a dispatched task carries a link the vendor opens to
//! accept, decline, say when they can come, upload photos and send an
//! invoice, with no account. Vendors can also be invited to Alpha.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE ticket_task
    ADD COLUMN IF NOT EXISTS vendor_token_hash text NULL,
    ADD COLUMN IF NOT EXISTS vendor_response text NULL,
    ADD COLUMN IF NOT EXISTS vendor_responded_at timestamptz NULL,
    ADD COLUMN IF NOT EXISTS vendor_note text NULL;
CREATE INDEX IF NOT EXISTS idx_ticket_task_vendor_token ON ticket_task (vendor_token_hash)
    WHERE vendor_token_hash IS NOT NULL;
ALTER TABLE counterparty
    ADD COLUMN IF NOT EXISTS alpha_invited_at timestamptz NULL;
"#;

const DOWN: &str = r#"
ALTER TABLE counterparty DROP COLUMN IF EXISTS alpha_invited_at;
DROP INDEX IF EXISTS idx_ticket_task_vendor_token;
ALTER TABLE ticket_task
    DROP COLUMN IF EXISTS vendor_note,
    DROP COLUMN IF EXISTS vendor_responded_at,
    DROP COLUMN IF EXISTS vendor_response,
    DROP COLUMN IF EXISTS vendor_token_hash;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(DOWN).await?;
        Ok(())
    }
}
