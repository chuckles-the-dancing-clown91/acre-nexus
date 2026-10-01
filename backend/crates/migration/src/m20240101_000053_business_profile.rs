//! **Business profile** (roadmap area 5): one row per workspace holding how
//! the business shows up to the public — contact details, hours, social
//! links — plus its Google Business Profile place and how its reviews are
//! displayed. Review text is never stored (Google's terms allow only the
//! place id); it is fetched and cached in memory.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS business_profile (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    business_name text NULL,
    phone text NULL,
    email text NULL,
    website text NULL,
    address text NULL,
    hours text NULL,
    description text NULL,
    facebook_url text NULL,
    instagram_url text NULL,
    yelp_url text NULL,
    nextdoor_url text NULL,
    google_place_id text NULL,
    google_place_name text NULL,
    google_review_url text NULL,
    show_reviews boolean NOT NULL DEFAULT true,
    min_rating integer NOT NULL DEFAULT 4,
    max_reviews integer NOT NULL DEFAULT 5,
    refresh_minutes integer NOT NULL DEFAULT 360,
    updated_by uuid NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS uq_business_profile_tenant ON business_profile (tenant_id);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        db.execute_unprepared(&format!(
            "ALTER TABLE business_profile ENABLE ROW LEVEL SECURITY; \
             ALTER TABLE business_profile FORCE ROW LEVEL SECURITY; \
             DROP POLICY IF EXISTS business_profile_tenant_isolation ON business_profile; \
             CREATE POLICY business_profile_tenant_isolation ON business_profile \
               USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS business_profile")
            .await?;
        Ok(())
    }
}
