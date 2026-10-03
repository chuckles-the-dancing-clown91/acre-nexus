//! **Maintenance as actions**: job kits are keyed (so a workspace's renamed or
//! retired kit stays that way while new kits still arrive), the symptom
//! starters ("AC not cooling") are retired in favour of action kits
//! ("Replace thermostat"), parts carry a link to where they're bought, and a
//! ticket comment can record an action ("On my way", "Waiting on parts").

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE issue_template ADD COLUMN IF NOT EXISTS kit_key text NULL;
UPDATE issue_template SET kit_key = 'replace-shower', name = 'Replace shower' WHERE seeded AND kit_key IS NULL AND name = 'Shower replacement';
UPDATE issue_template SET kit_key = 'replace-toilet', name = 'Replace toilet' WHERE seeded AND kit_key IS NULL AND name = 'Toilet replacement';
UPDATE issue_template SET kit_key = 'replace-water-heater', name = 'Replace water heater' WHERE seeded AND kit_key IS NULL AND name = 'Water heater replacement';
UPDATE issue_template SET kit_key = 'patch-drywall', name = 'Patch and paint drywall' WHERE seeded AND kit_key IS NULL AND name = 'Drywall patch and paint';
UPDATE issue_template SET kit_key = 'repaint-unit', name = 'Repaint a unit (turnover)' WHERE seeded AND kit_key IS NULL AND name = 'Unit turn: interior repaint';
UPDATE issue_template SET kit_key = 'service-hvac', name = 'Service HVAC (seasonal)' WHERE seeded AND kit_key IS NULL AND name = 'HVAC seasonal service';
-- The old symptom starters (no tasks, from the starter set) leave the catalog;
-- work orders and routines made from them keep what they have.
UPDATE issue_template SET active = false
 WHERE seeded AND kit_key IS NULL AND tasks = '[]'::jsonb;
CREATE INDEX IF NOT EXISTS idx_issue_template_key ON issue_template (tenant_id, kit_key);
ALTER TABLE ticket_part ADD COLUMN IF NOT EXISTS url text NULL;
ALTER TABLE ticket_comment ADD COLUMN IF NOT EXISTS action text NULL;
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
                "ALTER TABLE ticket_comment DROP COLUMN IF EXISTS action; \
                 ALTER TABLE ticket_part DROP COLUMN IF EXISTS url; \
                 DROP INDEX IF EXISTS idx_issue_template_key; \
                 ALTER TABLE issue_template DROP COLUMN IF EXISTS kit_key;",
            )
            .await?;
        Ok(())
    }
}
