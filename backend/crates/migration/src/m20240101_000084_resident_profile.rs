//! **Resident profile.** What a landlord needs to know about a resident beyond
//! the account profile: work, emergency contact, who lives there, pets as a
//! list, and where they rented before. The resident keeps it current (it fills
//! their applications and is their mobile ID card); property managers can edit
//! it too.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POLICY: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

const SQL: &str = r#"
CREATE TABLE IF NOT EXISTS resident_profile (
    user_id uuid PRIMARY KEY REFERENCES app_user(id) ON DELETE CASCADE,
    tenant_id uuid NOT NULL REFERENCES tenant(id) ON DELETE CASCADE,
    employer text NULL,
    job_title text NULL,
    employer_phone text NULL,
    emergency_contact_name text NULL,
    emergency_contact_phone text NULL,
    emergency_contact_relation text NULL,
    occupants jsonb NOT NULL DEFAULT '[]'::jsonb,
    pets jsonb NOT NULL DEFAULT '[]'::jsonb,
    prior_rentals jsonb NOT NULL DEFAULT '[]'::jsonb,
    staff_notes text NULL,
    updated_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS ix_resident_profile_tenant ON resident_profile (tenant_id);
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let c = manager.get_connection();
        c.execute_unprepared(SQL).await?;
        c.execute_unprepared(&format!(
            "ALTER TABLE resident_profile ENABLE ROW LEVEL SECURITY; \
             ALTER TABLE resident_profile FORCE ROW LEVEL SECURITY; \
             DROP POLICY IF EXISTS resident_profile_tenant_isolation ON resident_profile; \
             CREATE POLICY resident_profile_tenant_isolation ON resident_profile \
             USING ({POLICY}) WITH CHECK ({POLICY});"
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS resident_profile;")
            .await?;
        Ok(())
    }
}
