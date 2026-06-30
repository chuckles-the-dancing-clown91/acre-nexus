//! Tamper-evident lease signatures: record a SHA-256 hash of the document body
//! and the signer's IP at signing time, so a signed lease can be proven unchanged
//! (a real e-signature audit trail, vs just a typed name).

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
            col("signed_hash").string().null().take(),
            col("signed_ip").string().null().take(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("lease_document"))
                        .add_column_if_not_exists(&mut c)
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for c in ["signed_hash", "signed_ip"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("lease_document"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
