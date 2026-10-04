//! **Required items**: a maintenance plan can come from the code-required
//! catalog (`mandate_key`) and carry its own lead time for scheduling.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE maintenance_plan
    ADD COLUMN IF NOT EXISTS mandate_key text NULL,
    ADD COLUMN IF NOT EXISTS lead_days int NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_maintenance_plan_mandate
    ON maintenance_plan (property_id, mandate_key) WHERE mandate_key IS NOT NULL;
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
                "DROP INDEX IF EXISTS uq_maintenance_plan_mandate; \
                 ALTER TABLE maintenance_plan DROP COLUMN IF EXISTS lead_days, DROP COLUMN IF EXISTS mandate_key;",
            )
            .await?;
        Ok(())
    }
}
