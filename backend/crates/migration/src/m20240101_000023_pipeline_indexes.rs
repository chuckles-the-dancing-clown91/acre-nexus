//! **Pipeline hot-path indexes**: the white-glove flow looks vehicles up by
//! their owner (`vehicle.user_id`) on every portal application, and all three
//! intake doors look prior applications up by `(tenant_id, email)` for the
//! reuse check and the portal's "my applications" view.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_vehicle_user")
                    .table(Alias::new("vehicle"))
                    .col(Alias::new("user_id"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_application_email")
                    .table(Alias::new("application"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("email"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for name in ["idx_application_email", "idx_vehicle_user"] {
            manager
                .drop_index(
                    Index::drop()
                        .name(name)
                        .table(Alias::new(if name == "idx_vehicle_user" {
                            "vehicle"
                        } else {
                            "application"
                        }))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
