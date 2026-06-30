//! Tenancy & multi-entity spec — **Phase A: the entity model**.
//!
//! Implements the foundation from the Onboarding/Multi-Entity/Tenancy spec:
//!
//! * `tenant.parent_org_id` — nullable future-proofing for a holding company that
//!   groups several PM-brand tenants (no code depends on it yet).
//! * `llc` enrichment — the existing holding entity *is* the spec's
//!   `legal_entities`; we add `entity_type`, `registered_agent`, and `status` so
//!   each LLC carries its accounting/liability identity.
//! * `owner` — investors / members the firm tracks (the firm itself can be one).
//! * `entity_ownership` — the per-LLC **cap table** (owner → ownership %, role).
//! * `bank_account` — `operating` / `trust` accounts scoped to one LLC. Trust
//!   accounts carry the commingling invariant (enforced in the accounting layer).
//! * `portfolio` + `property.portfolio_id` — logical grouping of properties by
//!   investor / strategy / region.
//! * `user_role.scope` + `scope_ref_id` — the **scope dimension** on role
//!   assignments (`tenant` / `entity:{id}` / `portfolio:{id}` / `property:{id}`),
//!   resolved hierarchically by `rbac::scope_covers`.
//!
//! All new tenant-owned tables get a `tenant_id` and an RLS policy keyed on the
//! `app.tenant_id` session variable (defence in depth), mirroring
//! `m20240101_000002_rls`.

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

/// New tenant-scoped tables that need an RLS isolation policy.
const TENANT_SCOPED: &[&str] = &["owner", "entity_ownership", "bank_account", "portfolio"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- tenant: parent_org_id (holding-company future-proofing) ----
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("tenant"))
                    .add_column_if_not_exists(col("parent_org_id").uuid().null())
                    .to_owned(),
            )
            .await?;

        // ---- llc: richer legal-entity identity ----
        for mut c in [
            col("entity_type").string().not_null().default("llc").take(),
            col("registered_agent").string().null().take(),
            col("status").string().not_null().default("active").take(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("llc"))
                        .add_column_if_not_exists(&mut c)
                        .to_owned(),
                )
                .await?;
        }

        // ---- owner (investors / members; the firm itself may be one) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("owner"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    // `firm` | `individual` | `company`
                    .col(col("kind").string().not_null().default("individual"))
                    .col(col("name").string().not_null())
                    .col(col("email").string().null())
                    .col(col("phone").string().null())
                    .col(col("notes").text().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "owner", "tenant_id").await?;

        // ---- entity_ownership (the per-LLC cap table) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("entity_ownership"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    // FK to `llc.id` (the legal entity).
                    .col(col("entity_id").uuid().not_null())
                    .col(col("owner_id").uuid().not_null())
                    // Ownership stored as basis points (10000 = 100%) for exactness.
                    .col(col("ownership_bps").integer().not_null().default(0))
                    // `member` | `manager` | `investor`
                    .col(col("role").string().not_null().default("member"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "entity_ownership", "tenant_id").await?;
        index(manager, "entity_ownership", "entity_id").await?;

        // ---- bank_account (operating / trust, scoped to one LLC) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("bank_account"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("entity_id").uuid().not_null())
                    // `operating` | `trust`
                    .col(col("kind").string().not_null().default("operating"))
                    .col(col("institution").string().not_null())
                    .col(col("masked_number").string().null())
                    .col(col("status").string().not_null().default("active"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "bank_account", "tenant_id").await?;
        index(manager, "bank_account", "entity_id").await?;

        // ---- portfolio (grouping of properties by investor / strategy / region) ----
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("portfolio"))
                    .if_not_exists()
                    .col(uuid_pk())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("strategy").string().not_null().default(""))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "portfolio", "tenant_id").await?;

        // ---- property: optional portfolio grouping ----
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("property"))
                    .add_column_if_not_exists(col("portfolio_id").uuid().null())
                    .to_owned(),
            )
            .await?;

        // ---- user_role: the scope dimension on assignments ----
        // `scope` ∈ platform | tenant | entity | portfolio | property; `scope_ref_id`
        // points at the entity/portfolio/property when the scope is narrower than
        // the whole tenant. Existing rows default to a tenant/platform-wide grant.
        for mut c in [
            col("scope").string().not_null().default("tenant").take(),
            col("scope_ref_id").uuid().null().take(),
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("user_role"))
                        .add_column_if_not_exists(&mut c)
                        .to_owned(),
                )
                .await?;
        }
        // Backfill: a null tenant_id assignment is a platform-wide grant.
        manager
            .get_connection()
            .execute_unprepared("UPDATE user_role SET scope = 'platform' WHERE tenant_id IS NULL;")
            .await?;

        // ---- RLS on the new tenant-scoped tables ----
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

        for c in ["scope", "scope_ref_id"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("user_role"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("property"))
                    .drop_column(Alias::new("portfolio_id"))
                    .to_owned(),
            )
            .await?;
        for t in ["portfolio", "bank_account", "entity_ownership", "owner"] {
            manager
                .drop_table(Table::drop().table(Alias::new(t)).if_exists().to_owned())
                .await?;
        }
        for c in ["entity_type", "registered_agent", "status"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("llc"))
                        .drop_column(Alias::new(c))
                        .to_owned(),
                )
                .await?;
        }
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("tenant"))
                    .drop_column(Alias::new("parent_org_id"))
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
