//! **Website embeds**: a client puts Vantedge widgets (listings, tour form,
//! reviews, site maps) on their own website. Two settings on the business
//! profile: whether embedding is on, and which sites may show it.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE business_profile \
                   ADD COLUMN IF NOT EXISTS embed_enabled boolean NOT NULL DEFAULT true, \
                   ADD COLUMN IF NOT EXISTS embed_origins text NULL;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE business_profile DROP COLUMN IF EXISTS embed_enabled, \
                 DROP COLUMN IF EXISTS embed_origins;",
            )
            .await?;
        Ok(())
    }
}
