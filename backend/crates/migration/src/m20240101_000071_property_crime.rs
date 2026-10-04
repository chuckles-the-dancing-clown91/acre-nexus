//! **Crime statistics** on a property: the nearest reporting agency's offense
//! rates against the state and the country, from the FBI's Crime Data
//! Explorer, refreshed with the rest of the public records.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS property_crime (
    property_id uuid PRIMARY KEY REFERENCES property(id) ON DELETE CASCADE,
    tenant_id uuid NOT NULL,
    agency_ori text NULL,
    agency_name text NOT NULL,
    agency_km double precision NULL,
    period_from text NOT NULL,
    period_to text NOT NULL,
    population bigint NULL,
    offenses jsonb NOT NULL DEFAULT '[]'::jsonb,
    verdict text NOT NULL DEFAULT 'unknown',
    source text NOT NULL,
    fetched_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
"#;

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(UP).await?;
        let t = "property_crime";
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
            .execute_unprepared("DROP TABLE IF EXISTS property_crime;")
            .await?;
        Ok(())
    }
}
