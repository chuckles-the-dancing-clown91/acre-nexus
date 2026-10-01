//! **Search appearance** (roadmap: SEO): what a client's public site says to
//! search engines. A title and description for the home page, and the
//! Search Console verification token.

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
                   ADD COLUMN IF NOT EXISTS seo_title text NULL, \
                   ADD COLUMN IF NOT EXISTS seo_description text NULL, \
                   ADD COLUMN IF NOT EXISTS google_site_verification text NULL;",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE business_profile DROP COLUMN IF EXISTS seo_title, \
                 DROP COLUMN IF EXISTS seo_description, \
                 DROP COLUMN IF EXISTS google_site_verification;",
            )
            .await?;
        Ok(())
    }
}
