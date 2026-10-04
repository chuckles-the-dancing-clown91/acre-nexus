//! **Onboarding autofill**: the year built from the property record, so it
//! can be proposed for the property like beds, baths and square feet.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE property_detail ADD COLUMN IF NOT EXISTS year_built int NULL;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE property_detail DROP COLUMN IF EXISTS year_built;")
            .await?;
        Ok(())
    }
}
