//! **Two-way texts** (Vantedge phase 2).
//!
//! * `sms_thread` — one conversation per phone number per workspace, matched to
//!   a lease's resident (or a member) when the number is known. Carries the
//!   inbox state (`open` / `done`, unread count, last preview) and the number's
//!   **opt-out**: `opted_out_at` is set by STOP and cleared by START, and every
//!   outbound text to that number is refused while it's set.
//! * `sms_message` — the timeline: `in` (from the person) and `out` (typed in
//!   the console, or any notification text the platform sent to that number).
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

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("sms_thread"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    // E.164, e.g. +17605551234.
                    .col(col("phone").string().not_null())
                    // Who the number belongs to, when known.
                    .col(col("lease_id").uuid().null())
                    .col(col("user_id").uuid().null())
                    .col(col("display_name").string().null())
                    // open | done
                    .col(col("status").string().not_null().default("open"))
                    .col(col("unread_count").integer().not_null().default(0))
                    .col(col("last_preview").string().null())
                    .col(col("last_message_at").timestamp_with_time_zone().null())
                    .col(col("opted_out_at").timestamp_with_time_zone().null())
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
                    .name("uq_sms_thread_tenant_phone")
                    .table(Alias::new("sms_thread"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("phone"))
                    .to_owned(),
            )
            .await?;
        index(manager, "sms_thread", "last_message_at").await?;
        enforce_rls(manager, "sms_thread").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("sms_message"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("thread_id").uuid().not_null())
                    // in | out
                    .col(col("direction").string().not_null())
                    .col(col("body").text().not_null())
                    // received | queued | sent | failed | blocked
                    .col(col("status").string().not_null())
                    .col(col("provider_message_id").string().null())
                    // The notification template that produced an automatic text.
                    .col(col("template_key").string().null())
                    // The staff member who typed it (console replies).
                    .col(col("sent_by_user_id").uuid().null())
                    .col(col("media_count").integer().not_null().default(0))
                    .col(col("error").string().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "sms_message", "thread_id").await?;
        index(manager, "sms_message", "tenant_id").await?;
        enforce_rls(manager, "sms_message").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for t in ["sms_message", "sms_thread"] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
