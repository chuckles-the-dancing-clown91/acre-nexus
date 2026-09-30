//! **Property address + photo** (Vantedge phase 2C).
//!
//! A property gets a full postal address (`state`, `postal_code`) so autofill
//! can save what the map suggested, and a photo record: `photo_status`
//! (`none` | `stored` | `placeholder` | `failed`), when it was last tried and
//! why it failed, so the nightly `property_photo` job knows what to retry.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("property"))
                    .add_column(col("state").string().not_null().default(""))
                    .add_column(col("postal_code").string().not_null().default(""))
                    .add_column(col("photo_status").string().not_null().default("none"))
                    .add_column(col("photo_attempted_at").timestamp_with_time_zone().null())
                    .add_column(col("photo_error").string().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("property"))
                    .drop_column(Alias::new("state"))
                    .drop_column(Alias::new("postal_code"))
                    .drop_column(Alias::new("photo_status"))
                    .drop_column(Alias::new("photo_attempted_at"))
                    .drop_column(Alias::new("photo_error"))
                    .to_owned(),
            )
            .await
    }
}
