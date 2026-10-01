//! **The back office** (Vantedge phase 2B): people, time and money spent.
//!
//! * `employee_profile` — HR + pay details for a staff member (1:1 with a user
//!   in a workspace): employment type, pay rate, **bill rate** (what an hour of
//!   their work is charged to owners), dates, vehicle, emergency contact.
//! * `work_shift` — planned working time (work / on call / training).
//! * `time_off_request` — vacation / sick / personal / unpaid, reviewed.
//! * `time_entry` — the clock: hours against a work order, rehab project or
//!   property (or travel / shop / office time). The source of hours worked,
//!   payroll, labor cost and in-house maintenance billed to owners. Pay and
//!   bill rates are frozen when an entry closes. At most one open entry per
//!   person (partial unique index).
//! * `expense` — receipts and mileage, optionally billable to the owner.
//!
//! All tenant-owned, with the same enforced RLS as every other scoped table.

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

const TABLES: &[&str] = &[
    "expense",
    "time_entry",
    "time_off_request",
    "work_shift",
    "employee_profile",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("employee_profile"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("user_id").uuid().not_null())
                    .col(col("title").string().null())
                    // full_time | part_time | seasonal | contractor (1099)
                    .col(
                        col("employment_type")
                            .string()
                            .not_null()
                            .default("full_time"),
                    )
                    .col(col("pay_rate_cents").big_integer().not_null().default(0))
                    .col(col("bill_rate_cents").big_integer().not_null().default(0))
                    .col(col("hire_date").string().null())
                    .col(col("end_date").string().null())
                    .col(col("weekly_hours_target").integer().not_null().default(40))
                    // company | personal
                    .col(
                        col("default_vehicle")
                            .string()
                            .not_null()
                            .default("company"),
                    )
                    .col(col("mileage_reimbursed").boolean().not_null().default(true))
                    .col(col("emergency_contact_name").string().null())
                    .col(col("emergency_contact_phone").string().null())
                    .col(col("calendar_color").string().not_null().default("#0e7c86"))
                    .col(col("notes").text().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .unique()
                    .name("uq_employee_profile_tenant_user")
                    .table(Alias::new("employee_profile"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("user_id"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("work_shift"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("user_id").uuid().not_null())
                    .col(col("starts_at").timestamp_with_time_zone().not_null())
                    .col(col("ends_at").timestamp_with_time_zone().not_null())
                    // work | on_call | training
                    .col(col("kind").string().not_null().default("work"))
                    .col(col("property_id").uuid().null())
                    .col(col("notes").string().null())
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "work_shift", "user_id").await?;
        index(manager, "work_shift", "starts_at").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("time_off_request"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("user_id").uuid().not_null())
                    .col(col("starts_on").string().not_null())
                    // inclusive
                    .col(col("ends_on").string().not_null())
                    // vacation | sick | personal | unpaid
                    .col(col("kind").string().not_null().default("vacation"))
                    // pending | approved | denied | cancelled
                    .col(col("status").string().not_null().default("pending"))
                    .col(col("reason").text().null())
                    .col(col("reviewed_by").uuid().null())
                    .col(col("reviewed_at").timestamp_with_time_zone().null())
                    .col(col("review_note").string().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "time_off_request", "user_id").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("time_entry"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("user_id").uuid().not_null())
                    // work_order | project | property | travel | shop | admin | other
                    .col(col("kind").string().not_null().default("work_order"))
                    .col(col("maintenance_ticket_id").uuid().null())
                    .col(col("rehab_project_id").uuid().null())
                    .col(col("property_id").uuid().null())
                    .col(col("started_at").timestamp_with_time_zone().not_null())
                    .col(col("ended_at").timestamp_with_time_zone().null())
                    .col(col("break_minutes").integer().not_null().default(0))
                    .col(col("notes").string().null())
                    // Frozen when the entry closes.
                    .col(col("pay_rate_cents").big_integer().null())
                    .col(col("bill_rate_cents").big_integer().null())
                    .col(col("approved_by").uuid().null())
                    .col(col("approved_at").timestamp_with_time_zone().null())
                    // Missed punches (closed by the sweeper, held for review).
                    .col(col("missed_punch").boolean().not_null().default(false))
                    .col(col("missed_punch_reason").string().null())
                    .col(col("claimed_end").timestamp_with_time_zone().null())
                    .col(col("punch_note").string().null())
                    .col(col("resolved_by").uuid().null())
                    .col(col("resolved_at").timestamp_with_time_zone().null())
                    // Clock location (at punches only).
                    .col(col("in_lat").double().null())
                    .col(col("in_lng").double().null())
                    .col(col("in_distance_m").integer().null())
                    .col(col("out_lat").double().null())
                    .col(col("out_lng").double().null())
                    .col(col("out_distance_m").integer().null())
                    // The owner bill this time was charged on, once billed.
                    .col(col("billed_bill_id").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "time_entry", "user_id").await?;
        index(manager, "time_entry", "started_at").await?;
        index(manager, "time_entry", "maintenance_ticket_id").await?;
        index(manager, "time_entry", "rehab_project_id").await?;
        index(manager, "time_entry", "property_id").await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_time_entry_one_open \
                 ON time_entry (tenant_id, user_id) WHERE ended_at IS NULL; \
                 ALTER TABLE time_entry DROP CONSTRAINT IF EXISTS time_entry_ends_after_start; \
                 ALTER TABLE time_entry ADD CONSTRAINT time_entry_ends_after_start \
                   CHECK (ended_at IS NULL OR ended_at > started_at);",
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("expense"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("incurred_on").string().not_null())
                    // fuel | mileage | materials | equipment | repairs | vehicle | insurance
                    // | payroll | marketing | software | licenses | other
                    .col(col("category").string().not_null())
                    .col(col("vendor").string().null())
                    .col(col("description").string().not_null().default(""))
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    // Mileage: hundredths of a mile, and the rate applied.
                    .col(col("miles_hundredths").big_integer().null())
                    .col(col("mileage_rate_mills").big_integer().null())
                    .col(col("tax_deductible").boolean().not_null().default(true))
                    // company | personal | none
                    .col(col("vehicle").string().not_null().default("none"))
                    .col(col("reimbursable").boolean().not_null().default(false))
                    .col(col("reimbursed_at").timestamp_with_time_zone().null())
                    .col(col("billable_to_owner").boolean().not_null().default(false))
                    .col(col("billed_bill_id").uuid().null())
                    .col(col("user_id").uuid().null())
                    .col(col("maintenance_ticket_id").uuid().null())
                    .col(col("rehab_project_id").uuid().null())
                    .col(col("property_id").uuid().null())
                    .col(col("asset_id").uuid().null())
                    // odometer_start / odometer_end / round_trip / from / to / purpose
                    .col(col("details").json_binary().not_null().default("{}"))
                    .col(col("recorded_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "expense", "incurred_on").await?;
        index(manager, "expense", "maintenance_ticket_id").await?;
        index(manager, "expense", "rehab_project_id").await?;
        index(manager, "expense", "property_id").await?;
        index(manager, "expense", "user_id").await?;

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
