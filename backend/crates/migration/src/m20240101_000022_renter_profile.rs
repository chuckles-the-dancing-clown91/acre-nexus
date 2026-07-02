//! **Renter profile attributes** (white-glove applications): the durable facts
//! a rental application needs — pets, military status, stated income — live on
//! the person's profile, so applying through the renter portal auto-fills
//! everything and the tenant only ever has to keep their profile current.
//! (Vehicles already attach to the user via `vehicle.user_id`; government ID
//! and SSN already live here encrypted.)

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for mut c in [
            col("has_pet").boolean().not_null().default(false).take(),
            col("pet_details").text().null().take(),
            col("is_military")
                .boolean()
                .not_null()
                .default(false)
                .take(),
            col("annual_income_cents").big_integer().null().take(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("user_profile"))
                        .add_column_if_not_exists(&mut c)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for c in [
            "annual_income_cents",
            "is_military",
            "pet_details",
            "has_pet",
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("user_profile"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
