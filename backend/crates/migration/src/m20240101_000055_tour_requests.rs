//! **Tour requests** (roadmap area 6): a prospect asks to see a home from the
//! public site, with preferred times and consent, without a full application.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS tour_request (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    listing_id uuid NULL,
    name text NOT NULL,
    email text NOT NULL,
    phone text NULL,
    preferred_times text NULL,
    message text NULL,
    consent boolean NOT NULL DEFAULT false,
    status text NOT NULL DEFAULT 'new',
    note text NULL,
    handled_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_tour_request_tenant ON tour_request (tenant_id, status, created_at DESC);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        db.execute_unprepared(&format!(
            "ALTER TABLE tour_request ENABLE ROW LEVEL SECURITY; \
             ALTER TABLE tour_request FORCE ROW LEVEL SECURITY; \
             DROP POLICY IF EXISTS tour_request_tenant_isolation ON tour_request; \
             CREATE POLICY tour_request_tenant_isolation ON tour_request \
               USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS tour_request")
            .await?;
        Ok(())
    }
}
