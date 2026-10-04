//! **Appointments**: a visit someone has to be home for, or a showing.
//! Staff offer windows; the resident, prospect or vendor picks one from a
//! link or the portal; reminders go out before it.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS appointment (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    property_id uuid NOT NULL REFERENCES property(id) ON DELETE CASCADE,
    unit_id uuid NULL,
    kind text NOT NULL DEFAULT 'repair',
    subject_type text NOT NULL DEFAULT 'ticket',
    subject_id uuid NULL,
    title text NOT NULL,
    status text NOT NULL DEFAULT 'proposed',
    windows jsonb NOT NULL DEFAULT '[]'::jsonb,
    starts_at timestamptz NULL,
    ends_at timestamptz NULL,
    with_name text NULL,
    with_email text NULL,
    with_phone text NULL,
    with_role text NOT NULL DEFAULT 'resident',
    assignee_user_id uuid NULL,
    vendor_entity_id uuid NULL,
    note text NULL,
    access_notes text NULL,
    token_hash text NULL,
    confirmed_by text NULL,
    confirmed_at timestamptz NULL,
    proposed_start timestamptz NULL,
    proposed_end timestamptz NULL,
    reminded jsonb NOT NULL DEFAULT '[]'::jsonb,
    outcome_note text NULL,
    created_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_appointment_when ON appointment (tenant_id, starts_at);
CREATE INDEX IF NOT EXISTS idx_appointment_subject ON appointment (tenant_id, subject_type, subject_id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_appointment_token ON appointment (token_hash) WHERE token_hash IS NOT NULL;
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        let t = "appointment";
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
            .execute_unprepared("DROP TABLE IF EXISTS appointment;")
            .await?;
        Ok(())
    }
}
