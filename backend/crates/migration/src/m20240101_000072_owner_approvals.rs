//! **Owner approvals**: work over the owner's limit waits for their yes, and
//! finished billable work waits for their sign-off, from a link or the owner
//! portal. Owners can also hold a login.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE owner
    ADD COLUMN IF NOT EXISTS user_id uuid NULL,
    ADD COLUMN IF NOT EXISTS approval_limit_cents bigint NULL;
CREATE TABLE IF NOT EXISTS owner_approval (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    owner_id uuid NOT NULL REFERENCES owner(id) ON DELETE CASCADE,
    ticket_id uuid NOT NULL REFERENCES maintenance_ticket(id) ON DELETE CASCADE,
    kind text NOT NULL DEFAULT 'approval',
    amount_cents bigint NOT NULL DEFAULT 0,
    status text NOT NULL DEFAULT 'pending',
    token_hash text NULL,
    note text NULL,
    requested_by uuid NULL,
    requested_at timestamptz NOT NULL DEFAULT now(),
    decided_at timestamptz NULL,
    decided_by text NULL,
    decision_note text NULL,
    override_reason text NULL,
    nudged_at timestamptz NULL,
    nudges int NOT NULL DEFAULT 0,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_owner_approval_ticket ON owner_approval (tenant_id, ticket_id);
CREATE INDEX IF NOT EXISTS idx_owner_approval_owner ON owner_approval (tenant_id, owner_id, status);
CREATE UNIQUE INDEX IF NOT EXISTS uq_owner_approval_token ON owner_approval (token_hash) WHERE token_hash IS NOT NULL;
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        let t = "owner_approval";
        db.execute_unprepared(&format!(
            "ALTER TABLE {t} ENABLE ROW LEVEL SECURITY; \
             ALTER TABLE {t} FORCE ROW LEVEL SECURITY; \
             DROP POLICY IF EXISTS {t}_tenant_isolation ON {t}; \
             CREATE POLICY {t}_tenant_isolation ON {t} \
               USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TABLE IF EXISTS owner_approval; \
                 ALTER TABLE owner DROP COLUMN IF EXISTS approval_limit_cents, DROP COLUMN IF EXISTS user_id;",
            )
            .await?;
        Ok(())
    }
}
