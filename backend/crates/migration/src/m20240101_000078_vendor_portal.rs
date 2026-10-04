//! **Vendor portal** (fix plan F13): which signed-in users are a vendor
//! (counterparty) of this workspace. A vendor login sees only the work sent
//! to that vendor.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS vendor_portal_user (
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES app_user(id) ON DELETE CASCADE,
    counterparty_id uuid NOT NULL REFERENCES counterparty(id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, user_id)
);
CREATE INDEX IF NOT EXISTS ix_vendor_portal_user_cp ON vendor_portal_user (counterparty_id);
ALTER TABLE vendor_portal_user ENABLE ROW LEVEL SECURITY;
ALTER TABLE vendor_portal_user FORCE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS vendor_portal_user_tenant_isolation ON vendor_portal_user;
CREATE POLICY vendor_portal_user_tenant_isolation ON vendor_portal_user
    USING (NULLIF(current_setting('app.tenant_id', true), '') IS NULL
           OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''))
    WITH CHECK (NULLIF(current_setting('app.tenant_id', true), '') IS NULL
           OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), ''));
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
            .execute_unprepared("DROP TABLE IF EXISTS vendor_portal_user;")
            .await?;
        Ok(())
    }
}
