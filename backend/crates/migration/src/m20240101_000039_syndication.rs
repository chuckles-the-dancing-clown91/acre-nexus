//! **Investor / syndication** (issue #13, Beyond-GA vertical):
//!
//! * `investor_commitment` — an LP/GP's committed capital in a legal entity,
//!   with running `contributed` / `returned` balances.
//! * `capital_call` / `capital_call_line` — a call for capital, split pro-rata.
//! * `distribution` / `distribution_line` — a cash distribution run through the
//!   waterfall (see `api::syndication`), broken out by tier per investor.
//!
//! Tenant-owned; RLS is enforced with the empty-string-safe predicate (see
//! migration `000038`).

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

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("investor_commitment"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("entity_id").uuid().not_null())
                    .col(col("owner_id").uuid().not_null())
                    .col(col("role").string().not_null().default("investor"))
                    .col(col("committed_cents").big_integer().not_null().default(0))
                    .col(col("contributed_cents").big_integer().not_null().default(0))
                    .col(col("returned_cents").big_integer().not_null().default(0))
                    .col(col("status").string().not_null().default("active"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "investor_commitment", "tenant_id").await?;
        index(manager, "investor_commitment", "entity_id").await?;
        enforce_rls(manager, "investor_commitment").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("capital_call"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("entity_id").uuid().not_null())
                    .col(col("number").integer().not_null().default(1))
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("status").string().not_null().default("open"))
                    .col(col("due_date").string().null())
                    .col(col("memo").text().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "capital_call", "tenant_id").await?;
        index(manager, "capital_call", "entity_id").await?;
        enforce_rls(manager, "capital_call").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("capital_call_line"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("call_id").uuid().not_null())
                    .col(col("commitment_id").uuid().not_null())
                    .col(col("owner_id").uuid().not_null())
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("status").string().not_null().default("pending"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "capital_call_line", "tenant_id").await?;
        index(manager, "capital_call_line", "call_id").await?;
        enforce_rls(manager, "capital_call_line").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("distribution"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("entity_id").uuid().not_null())
                    .col(col("number").integer().not_null().default(1))
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("pref_rate_bps").integer().not_null().default(0))
                    .col(col("carry_bps").integer().not_null().default(0))
                    .col(col("status").string().not_null().default("final"))
                    .col(col("memo").text().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "distribution", "tenant_id").await?;
        index(manager, "distribution", "entity_id").await?;
        enforce_rls(manager, "distribution").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("distribution_line"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("distribution_id").uuid().not_null())
                    .col(col("commitment_id").uuid().not_null())
                    .col(col("owner_id").uuid().not_null())
                    .col(
                        col("return_of_capital_cents")
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(col("preferred_cents").big_integer().not_null().default(0))
                    .col(col("profit_cents").big_integer().not_null().default(0))
                    .col(col("carry_cents").big_integer().not_null().default(0))
                    .col(col("total_cents").big_integer().not_null().default(0))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "distribution_line", "tenant_id").await?;
        index(manager, "distribution_line", "distribution_id").await?;
        enforce_rls(manager, "distribution_line").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in [
            "distribution_line",
            "distribution",
            "capital_call_line",
            "capital_call",
            "investor_commitment",
        ] {
            db.execute_unprepared(&format!(
                "DROP POLICY IF EXISTS {table}_tenant_isolation ON {table};"
            ))
            .await?;
            manager
                .drop_table(
                    Table::drop()
                        .table(Alias::new(table))
                        .if_exists()
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
