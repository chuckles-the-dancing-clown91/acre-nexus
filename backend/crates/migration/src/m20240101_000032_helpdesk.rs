//! **Helpdesk & maintenance operations** (Phase 6, issue #10).
//!
//! * `maintenance_ticket` gains the SLA/lifecycle timestamps: when the first
//!   staff response landed, when the ticket resolved, and the response /
//!   resolution targets stamped from the tenant's SLA policy.
//! * `ticket_quote` — a contractor's quote on a work order: amount +
//!   description, approved or rejected by the same people who approve vendor
//!   bills; approval feeds the ticket's cost (and from there the vendor-bill
//!   prefill).
//! * `maintenance_plan` — preventive-maintenance schedule: a recurring task
//!   (e.g. HVAC service) that auto-opens a ticket every `cadence_days`.
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
        // SLA / lifecycle timestamps on the work order itself.
        for column in [
            "first_response_at",
            "resolved_at",
            "sla_response_due_at",
            "sla_resolve_due_at",
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("maintenance_ticket"))
                        .add_column_if_not_exists(
                            ColumnDef::new(Alias::new(column))
                                .timestamp_with_time_zone()
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("ticket_quote"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("ticket_id").uuid().not_null())
                    // The quoting contractor (counterparty).
                    .col(col("entity_id").uuid().not_null())
                    .col(col("description").string().not_null())
                    .col(col("amount_cents").big_integer().not_null())
                    // pending | approved | rejected
                    .col(col("status").string().not_null().default("pending"))
                    .col(col("decided_by").uuid().null())
                    .col(col("decided_at").timestamp_with_time_zone().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "ticket_quote", "tenant_id").await?;
        index(manager, "ticket_quote", "ticket_id").await?;
        enforce_rls(manager, "ticket_quote").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("maintenance_plan"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("property_id").uuid().not_null())
                    .col(col("unit_id").uuid().null())
                    .col(col("title").string().not_null())
                    .col(col("description").text().null())
                    .col(col("category").string().not_null().default("general"))
                    .col(col("priority").string().not_null().default("normal"))
                    // How often a ticket is generated.
                    .col(col("cadence_days").integer().not_null())
                    // ISO date the next ticket opens.
                    .col(col("next_due_date").string().not_null())
                    .col(col("active").boolean().not_null().default(true))
                    // The most recent auto-opened ticket.
                    .col(col("last_ticket_id").uuid().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "maintenance_plan", "tenant_id").await?;
        index(manager, "maintenance_plan", "property_id").await?;
        enforce_rls(manager, "maintenance_plan").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in ["maintenance_plan", "ticket_quote"] {
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
        for column in [
            "sla_resolve_due_at",
            "sla_response_due_at",
            "resolved_at",
            "first_response_at",
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("maintenance_ticket"))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
