//! Tenancy spec — **Phase D (routing) + the onboarding state machine (§9)**.
//!
//! * `domain` — maps an inbound `Host` to a tenant **and an audience**
//!   (`admin` / `owner` / `renter`). A single tenant can map many domains: an
//!   admin app, an owner portal, and a renter portal, each its own hostname.
//!   Custom domains carry a `verification_token` (TXT record) and a `tls_status`.
//!   The Rocket resolution guard (`tenancy::PublicTenant` / a host fairing) reads
//!   `Host`, looks the row up, sets `app.tenant_id` for RLS, and attaches the
//!   audience + theme to the request.
//! * `onboarding_workflow` — one resumable, audited setup workflow per tenant. Its
//!   `state` and per-step `steps` JSON drive the firm-admin onboarding checklist
//!   (branding → domains → entities → banking → portfolio → staff → live).
//!
//! Both are tenant-owned, so both get an RLS isolation policy.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
}

fn uuid_pk() -> ColumnDef {
    ColumnDef::new(Alias::new("id"))
        .uuid()
        .not_null()
        .primary_key()
        .take()
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

const TENANT_SCOPED: &[&str] = &["domain", "onboarding_workflow"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- domain (Host -> tenant + audience) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("domain"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("hostname").string().not_null())
                    // `subdomain` | `custom`
                    .col(col("kind").string().not_null().default("subdomain"))
                    // `admin` | `owner` | `renter`
                    .col(col("audience").string().not_null().default("admin"))
                    .col(col("verification_token").string().null())
                    .col(col("verified_at").timestamp_with_time_zone().null())
                    // `pending` | `active` | `failed`
                    .col(col("tls_status").string().not_null().default("pending"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq_domain_hostname")
                    .table(Alias::new("domain"))
                    .col(Alias::new("hostname"))
                    .unique()
                    .to_owned(),
            )
            .await?;
        index(manager, "domain", "tenant_id").await?;

        // ---- onboarding_workflow (one per tenant) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("onboarding_workflow"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    // The current furthest-reached state (see routes::onboarding::state).
                    .col(col("state").string().not_null().default("provisioning"))
                    // Per-step completion + metadata, recomputed on read.
                    .col(col("steps").json_binary().not_null().default(Expr::cust("'{}'::jsonb")))
                    .col(ts("created_at"))
                    .col(ts("updated_at"))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq_onboarding_workflow_tenant")
                    .table(Alias::new("onboarding_workflow"))
                    .col(Alias::new("tenant_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        // ---- RLS ----
        let db = manager.get_connection();
        for t in TENANT_SCOPED {
            let sql = format!(
                "ALTER TABLE {t} ENABLE ROW LEVEL SECURITY; \
                 DROP POLICY IF EXISTS {t}_tenant_isolation ON {t}; \
                 CREATE POLICY {t}_tenant_isolation ON {t} \
                   USING (\
                     current_setting('app.tenant_id', true) IS NULL \
                     OR tenant_id::text = current_setting('app.tenant_id', true)\
                   );"
            );
            db.execute_unprepared(&sql).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for t in TENANT_SCOPED {
            let sql = format!(
                "DROP POLICY IF EXISTS {t}_tenant_isolation ON {t}; \
                 ALTER TABLE {t} DISABLE ROW LEVEL SECURITY;"
            );
            db.execute_unprepared(&sql).await?;
        }
        for t in ["onboarding_workflow", "domain"] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        Ok(())
    }
}
