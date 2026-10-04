//! **Appliance profile.** An appliance (asset) gains the warranty details a
//! claim needs (start, policy number, phone, what is covered, whether it
//! transfers), its care instructions, a link to the manual, and when it was
//! last checked for recalls.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const SQL: &str = r#"
ALTER TABLE asset
    ADD COLUMN IF NOT EXISTS warranty_starts_on text NULL,
    ADD COLUMN IF NOT EXISTS warranty_policy_number text NULL,
    ADD COLUMN IF NOT EXISTS warranty_phone text NULL,
    ADD COLUMN IF NOT EXISTS warranty_coverage text NULL,
    ADD COLUMN IF NOT EXISTS warranty_transferable boolean NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS care_instructions text NULL,
    ADD COLUMN IF NOT EXISTS manual_url text NULL,
    ADD COLUMN IF NOT EXISTS recall_checked_on text NULL;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(SQL).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE asset DROP COLUMN IF EXISTS warranty_starts_on, \
                 DROP COLUMN IF EXISTS warranty_policy_number, DROP COLUMN IF EXISTS warranty_phone, \
                 DROP COLUMN IF EXISTS warranty_coverage, DROP COLUMN IF EXISTS warranty_transferable, \
                 DROP COLUMN IF EXISTS care_instructions, DROP COLUMN IF EXISTS manual_url, \
                 DROP COLUMN IF EXISTS recall_checked_on;",
            )
            .await?;
        Ok(())
    }
}
