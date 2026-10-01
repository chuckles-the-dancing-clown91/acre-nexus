//! **Alpha ↔ Vantedge** (Vantedge phase 2C): a vendor on Alpha Power Wash
//! linked to a counterparty here, and the work orders sent to it.
//!
//! * `counterparty` gains the link: which partner system (`alpha`), its base
//!   URL, when it was linked and whether the last call worked. The API key
//!   itself lives in the secrets vault (`partner.<id>.api_key`).
//! * `maintenance_ticket` gains the partner's job id and status, so the work
//!   order shows what the vendor's system says.

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
                    .table(Alias::new("counterparty"))
                    .add_column(col("partner_kind").string().null())
                    .add_column(col("partner_base_url").string().null())
                    .add_column(col("partner_linked_at").timestamp_with_time_zone().null())
                    .add_column(col("partner_status").string().null())
                    .add_column(col("partner_error").string().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("maintenance_ticket"))
                    .add_column(col("partner_counterparty_id").uuid().null())
                    .add_column(col("partner_job_id").string().null())
                    .add_column(col("partner_status").string().null())
                    .add_column(col("partner_synced_at").timestamp_with_time_zone().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_maintenance_ticket_partner_job_id")
                    .table(Alias::new("maintenance_ticket"))
                    .col(Alias::new("partner_job_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("maintenance_ticket"))
                    .drop_column(Alias::new("partner_counterparty_id"))
                    .drop_column(Alias::new("partner_job_id"))
                    .drop_column(Alias::new("partner_status"))
                    .drop_column(Alias::new("partner_synced_at"))
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("counterparty"))
                    .drop_column(Alias::new("partner_kind"))
                    .drop_column(Alias::new("partner_base_url"))
                    .drop_column(Alias::new("partner_linked_at"))
                    .drop_column(Alias::new("partner_status"))
                    .drop_column(Alias::new("partner_error"))
                    .to_owned(),
            )
            .await
    }
}
