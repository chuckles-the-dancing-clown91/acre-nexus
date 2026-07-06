//! **Vendor API outbound webhooks** (issue #68) — subscribe, don't poll.
//!
//! * `webhook_subscription` — a vendor token's registration: callback URL +
//!   the event types it wants, gated by the same scopes as its read access.
//!   The signing secret lives in the vault (`webhook_sub.<id>.secret`) and is
//!   returned exactly once at creation, like an API token.
//! * `webhook_delivery` — one event → one subscriber: payload, attempt count,
//!   status (`pending → delivered | dead`), the last error, and the response
//!   status — the observability surface vendors read (and replay from).
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
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("webhook_subscription"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    // The vendor token that owns this subscription.
                    .col(col("api_token_id").uuid().not_null())
                    .col(col("url").string().not_null())
                    // Event-type strings, e.g. `["listing.updated"]`.
                    .col(col("event_types").json_binary().not_null().default("[]"))
                    // Vault key of the HMAC signing secret.
                    .col(col("secret_ref").string().not_null())
                    .col(col("enabled").boolean().not_null().default(true))
                    .col(col("description").string().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "webhook_subscription", "tenant_id").await?;
        index(manager, "webhook_subscription", "api_token_id").await?;
        enforce_rls(manager, "webhook_subscription").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("webhook_delivery"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("subscription_id").uuid().not_null())
                    .col(col("event_type").string().not_null())
                    .col(col("payload").json_binary().not_null().default("{}"))
                    // pending | delivered | dead
                    .col(col("status").string().not_null().default("pending"))
                    .col(col("attempts").integer().not_null().default(0))
                    // Last HTTP status from the subscriber, when one answered.
                    .col(col("response_status").integer().null())
                    .col(col("last_error").text().null())
                    .col(col("delivered_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "webhook_delivery", "tenant_id").await?;
        index(manager, "webhook_delivery", "subscription_id").await?;
        enforce_rls(manager, "webhook_delivery").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in ["webhook_delivery", "webhook_subscription"] {
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
