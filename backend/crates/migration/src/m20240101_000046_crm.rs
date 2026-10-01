//! **CRM** (Vantedge phase 2B): owners are the property manager's clients.
//!
//! * `crm_note` — a timeline entry (note / call / email / meeting / issue /
//!   text / update) about an owner, an owner lead, a vendor or a property,
//!   optionally pinned, optionally with a **follow-up** date that stays open
//!   until someone marks it done.
//! * `owner_lead` — the pipeline for winning new management contracts:
//!   new → contacted → proposal → won / lost, by source, with the doors and
//!   the fee at stake; a won lead becomes an owner.
//!
//! Tenant-owned, with the same enforced RLS as every other scoped table.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

fn ts(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name))
        .timestamp_with_time_zone()
        .not_null()
        .default(Expr::current_timestamp())
        .take()
}

async fn index(manager: &SchemaManager<'_>, table: &str, column: &str) -> Result<(), DbErr> {
    manager
        .create_index(
            Index::create()
                .if_not_exists()
                .name(format!("idx_{table}_{column}"))
                .table(Alias::new(table))
                .col(Alias::new(column))
                .to_owned(),
        )
        .await
}

/// The isolation predicate, empty-GUC safe (see migration `000038`).
const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

async fn enforce_rls(manager: &SchemaManager<'_>, table: &str) -> Result<(), DbErr> {
    let policy = format!("{table}_tenant_isolation");
    let sql = format!(
        "ALTER TABLE {table} ENABLE ROW LEVEL SECURITY; \
         ALTER TABLE {table} FORCE ROW LEVEL SECURITY; \
         DROP POLICY IF EXISTS {policy} ON {table}; \
         CREATE POLICY {policy} ON {table} \
           USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
    );
    manager.get_connection().execute_unprepared(&sql).await?;
    Ok(())
}

const TABLES: &[&str] = &["crm_note", "owner_lead"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("crm_note"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    // owner | owner_lead | counterparty | property
                    .col(col("subject_type").string().not_null())
                    .col(col("subject_id").uuid().not_null())
                    .col(col("property_id").uuid().null())
                    // note | call | email | meeting | issue | text | update
                    .col(col("kind").string().not_null().default("note"))
                    .col(col("body").text().not_null())
                    .col(col("pinned").boolean().not_null().default(false))
                    .col(col("follow_up_on").string().null())
                    .col(col("follow_up_done_at").timestamp_with_time_zone().null())
                    .col(col("follow_up_done_by").uuid().null())
                    .col(col("author_id").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "crm_note", "subject_id").await?;
        index(manager, "crm_note", "follow_up_on").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("owner_lead"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("company").string().null())
                    .col(col("email").string().null())
                    .col(col("phone").string().null())
                    .col(col("address").string().null())
                    .col(col("properties_count").integer().not_null().default(1))
                    .col(col("doors").integer().not_null().default(1))
                    // website | referral | phone | email | event | mailer | other
                    .col(col("source").string().not_null().default("website"))
                    // new | contacted | proposal | won | lost
                    .col(col("status").string().not_null().default("new"))
                    .col(col("lost_reason").string().null())
                    .col(
                        col("monthly_rent_cents")
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(col("fee_bps").integer().null())
                    .col(col("notes").text().null())
                    .col(col("assigned_to").uuid().null())
                    .col(col("owner_id").uuid().null())
                    .col(col("won_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "owner_lead", "status").await?;

        for t in TABLES {
            enforce_rls(manager, t).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in TABLES {
            manager
                .drop_table(Table::drop().table(Alias::new(*t)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
