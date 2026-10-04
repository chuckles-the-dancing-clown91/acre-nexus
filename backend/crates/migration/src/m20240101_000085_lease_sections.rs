//! A lease document keeps its structure (articles and addenda) beside the
//! signable text, so the lease page can lay it out as a real agreement.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE lease_document ADD COLUMN IF NOT EXISTS sections jsonb NULL;")
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE lease_document DROP COLUMN IF EXISTS sections;")
            .await?;
        Ok(())
    }
}
