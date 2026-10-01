//! **Text tools** (fix plan F16–F17): who owns a conversation, photos sent by
//! text filed as documents, saved replies, and separate consent for marketing
//! texts.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE sms_thread ADD COLUMN IF NOT EXISTS assigned_user_id uuid NULL;
ALTER TABLE sms_thread ADD COLUMN IF NOT EXISTS marketing_opt_in_at timestamptz NULL;
ALTER TABLE sms_message ADD COLUMN IF NOT EXISTS media jsonb NOT NULL DEFAULT '[]'::jsonb;

CREATE TABLE IF NOT EXISTS text_saved_reply (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    title text NOT NULL,
    body text NOT NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_text_saved_reply_tenant ON text_saved_reply (tenant_id);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        let t = "text_saved_reply";
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
                "DROP TABLE IF EXISTS text_saved_reply; \
                 ALTER TABLE sms_message DROP COLUMN IF EXISTS media; \
                 ALTER TABLE sms_thread DROP COLUMN IF EXISTS marketing_opt_in_at; \
                 ALTER TABLE sms_thread DROP COLUMN IF EXISTS assigned_user_id;",
            )
            .await?;
        Ok(())
    }
}
