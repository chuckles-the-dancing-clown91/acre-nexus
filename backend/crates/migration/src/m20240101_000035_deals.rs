//! **Acquisition deal pipeline** (roadmap Phase 7, issues #41/#42) — the
//! buy-side domain behind the `flips` module:
//!
//! * `deal` — a prospective property moving `prospecting → offer →
//!   under_contract → closing → owned`, carrying its offer terms, underwriting
//!   assumptions (purchase / rehab / rent / financing / projection knobs), and a
//!   JSON due-diligence checklist. Converts into an owned `property`.
//! * `deal_event` — the deal's timeline (stage changes, offers, notes,
//!   conversion), mirroring `workflow_event`.
//!
//! Due-diligence files ride the existing polymorphic `document` service with
//! `owner_type = "deal"`. Tenant-owned, with enforced RLS like every scoped
//! table.

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

const RLS_PRED: &str = "current_setting('app.tenant_id', true) IS NULL \
     OR tenant_id::text = current_setting('app.tenant_id', true)";

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
                    .table(Alias::new("deal"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("address").string().not_null().default(""))
                    .col(col("city").string().not_null().default(""))
                    // prospecting | offer | under_contract | closing | owned | dead
                    .col(col("stage").string().not_null().default("prospecting"))
                    // flip | brrrr | rental | hold | wholesale
                    .col(col("strategy").string().not_null().default("flip"))
                    .col(col("property_type").string().null())
                    .col(col("source").string().null())
                    .col(col("broker_id").uuid().null())
                    .col(col("notes").text().null())
                    // Offer terms
                    .col(col("asking_price_cents").big_integer().null())
                    .col(col("offer_price_cents").big_integer().null())
                    .col(col("earnest_money_cents").big_integer().null())
                    .col(col("target_close_on").string().null())
                    // Underwriting assumptions
                    .col(col("arv_cents").big_integer().null())
                    .col(col("rehab_budget_cents").big_integer().null())
                    .col(col("closing_costs_cents").big_integer().null())
                    .col(col("est_monthly_rent_cents").big_integer().null())
                    .col(col("est_monthly_expenses_cents").big_integer().null())
                    .col(col("vacancy_bps").integer().null())
                    .col(col("down_payment_bps").integer().null())
                    .col(col("interest_rate_bps").integer().null())
                    .col(col("loan_term_years").integer().null())
                    .col(col("rent_growth_bps").integer().null())
                    .col(col("appreciation_bps").integer().null())
                    .col(col("exit_cap_rate_bps").integer().null())
                    .col(col("selling_costs_bps").integer().null())
                    .col(col("hold_years").integer().null())
                    .col(col("checklist").json_binary().not_null().default("[]"))
                    .col(col("converted_property_id").uuid().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "deal", "tenant_id").await?;
        index(manager, "deal", "stage").await?;
        enforce_rls(manager, "deal").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("deal_event"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("deal_id").uuid().not_null())
                    // created | stage_change | offer | note | converted
                    .col(col("kind").string().not_null().default("note"))
                    .col(col("from_stage").string().null())
                    .col(col("to_stage").string().null())
                    .col(col("body").text().null())
                    .col(col("actor_user_id").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "deal_event", "tenant_id").await?;
        index(manager, "deal_event", "deal_id").await?;
        enforce_rls(manager, "deal_event").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in ["deal_event", "deal"] {
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
