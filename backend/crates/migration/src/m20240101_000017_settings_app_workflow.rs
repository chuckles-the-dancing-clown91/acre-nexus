//! **System settings** + **application workflow history**.
//!
//! * `setting` — a per-tenant key/value store (JSON values) backing the typed
//!   settings catalog in `crate::settings`. One row per (tenant, key); absence
//!   means "use the catalog default". This is the general per-firm configuration
//!   store (the application-reuse policy is its first tenant).
//! * `application_event` — the immutable transition history for a rental
//!   application's workflow (mirrors `workflow_event` for properties), so the
//!   applications pipeline is auditable and resumable.
//!
//! Both are tenant-owned, so they get the same enforced RLS as every other
//! tenant table (ENABLE + FORCE + USING/WITH CHECK on `app.tenant_id`), matching
//! `m20240101_000015_rls_enforce`.

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
        // ---- setting (per-tenant config key/value) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("setting"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("key").string().not_null())
                    // JSON so a value can be a bool / number / string / object.
                    .col(col("value").json_binary().not_null())
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "setting", "tenant_id").await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq_setting_tenant_key")
                    .table(Alias::new("setting"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("key"))
                    .unique()
                    .to_owned(),
            )
            .await?;
        enforce_rls(manager, "setting").await?;

        // ---- application_event (workflow transition history) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("application_event"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("application_id").uuid().not_null())
                    .col(col("from_status").string().null())
                    .col(col("to_status").string().not_null())
                    .col(col("note").text().null())
                    .col(col("actor_user_id").uuid().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "application_event", "tenant_id").await?;
        index(manager, "application_event", "application_id").await?;
        enforce_rls(manager, "application_event").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "DROP POLICY IF EXISTS application_event_tenant_isolation ON application_event; \
             DROP POLICY IF EXISTS setting_tenant_isolation ON setting;",
        )
        .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("application_event"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("setting"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
