//! **Notification delivery expansion**: tenant-configurable delivery
//! providers, Web Push subscriptions, and the in-app inbox.
//!
//! * `notification_provider` — a tenant's configured delivery service for one
//!   channel (`email`: Resend/SendGrid/Postmark, `sms`: Twilio, `chat`:
//!   Slack/Discord). Non-secret settings live in `config` (JSON); the API
//!   credential lives in the secrets vault under `secret_ref`. One provider
//!   per channel is the default (partial unique index).
//! * `push_subscription` — one browser Web Push subscription (endpoint +
//!   client keys) per row, owned by a user.
//! * `notification.user_id` / `read_at` — notifications addressed to a user
//!   power the in-app inbox; `read_at` is the unread marker.
//!
//! Both new tables are tenant-owned with the same enforced RLS as every other
//! tenant table.

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
        // ---- notification_provider (tenant-configured delivery services) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("notification_provider"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("channel").string().not_null())
                    .col(col("kind").string().not_null())
                    // Non-secret settings (from address, account sid, …).
                    .col(col("config").json_binary().not_null().default("{}"))
                    // Secrets-vault key holding the API credential, if any.
                    .col(col("secret_ref").string().null())
                    .col(col("enabled").boolean().not_null().default(true))
                    .col(col("is_default").boolean().not_null().default(false))
                    .col(col("created_by").uuid().null())
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "notification_provider", "tenant_id").await?;
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_notification_provider_default \
                   ON notification_provider (tenant_id, channel) WHERE is_default;",
            )
            .await?;
        enforce_rls(manager, "notification_provider").await?;

        // ---- push_subscription (browser Web Push registrations) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("push_subscription"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("user_id").uuid().not_null())
                    .col(col("endpoint").text().not_null().unique_key())
                    // Client public key + auth secret, base64url as the browser
                    // hands them over.
                    .col(col("p256dh").text().not_null())
                    .col(col("auth").text().not_null())
                    .col(col("user_agent").text().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "push_subscription", "tenant_id").await?;
        index(manager, "push_subscription", "user_id").await?;
        enforce_rls(manager, "push_subscription").await?;

        // ---- notification: in-app inbox columns ----
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE notification ADD COLUMN IF NOT EXISTS user_id uuid; \
                 ALTER TABLE notification ADD COLUMN IF NOT EXISTS read_at timestamptz; \
                 CREATE INDEX IF NOT EXISTS idx_notification_user \
                   ON notification (tenant_id, user_id, read_at);",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            "DROP POLICY IF EXISTS push_subscription_tenant_isolation ON push_subscription; \
             DROP POLICY IF EXISTS notification_provider_tenant_isolation ON notification_provider; \
             DROP INDEX IF EXISTS idx_notification_user; \
             ALTER TABLE notification DROP COLUMN IF EXISTS read_at; \
             ALTER TABLE notification DROP COLUMN IF EXISTS user_id;",
        )
        .await?;
        for table in ["push_subscription", "notification_provider"] {
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
