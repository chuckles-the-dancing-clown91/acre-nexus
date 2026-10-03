//! **The property's story**: a description and its features (interior,
//! exterior, construction, utilities, community), like a listing's
//! "Facts and features".

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
ALTER TABLE property_detail ADD COLUMN IF NOT EXISTS description text NULL;
ALTER TABLE property_detail ADD COLUMN IF NOT EXISTS features jsonb NOT NULL DEFAULT '{}'::jsonb;
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
                "ALTER TABLE property_detail DROP COLUMN IF EXISTS features; \
                 ALTER TABLE property_detail DROP COLUMN IF EXISTS description;",
            )
            .await?;
        Ok(())
    }
}
