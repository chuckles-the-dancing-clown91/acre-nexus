//! **Leasing pipeline** (listing → application → screening → lease → signing):
//! the application row learns where it came from and what screening found.
//!
//! * `source` — which intake door created it: `public` (anonymous website),
//!   `portal` (an authenticated renter applying through their profile), or
//!   `back_office` (staff intake on an applicant's behalf).
//! * `applicant_user_id` — the platform user who applied, when the applicant
//!   is authenticated (powers "my applications" in the renter portal).
//! * `screening_status` / `screened_at` — the background check's outcome
//!   (`cleared` / `flagged`) recorded on the application itself, so the
//!   pipeline (and the auto-approve setting) can act on it.

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
            col("source").string().not_null().default("public").take(),
            col("applicant_user_id").uuid().null().take(),
            col("screening_status").string().null().take(),
            col("screened_at").timestamp_with_time_zone().null().take(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("application"))
                        .add_column_if_not_exists(&mut c)
                        .to_owned(),
                )
                .await?;
        }
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_application_applicant_user")
                    .table(Alias::new("application"))
                    .col(Alias::new("applicant_user_id"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP INDEX IF EXISTS idx_application_applicant_user;")
            .await?;
        for c in [
            "screened_at",
            "screening_status",
            "applicant_user_id",
            "source",
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("application"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
