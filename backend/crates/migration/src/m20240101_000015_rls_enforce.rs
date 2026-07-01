//! **Activate** row-level security as the enforced second wall for tenant
//! isolation (§ tenancy). Migration `000002` scaffolded `ENABLE ROW LEVEL
//! SECURITY` on a handful of tables with a `USING`-only policy — but because the
//! API connects as the tables' *owner*, plain `ENABLE` is bypassed and the
//! policies never bite, and without `WITH CHECK` a cross-tenant INSERT/UPDATE was
//! still possible. This migration makes RLS actually enforce:
//!
//!   * `FORCE ROW LEVEL SECURITY` on every tenant-owned table, so even the owner
//!     role the API uses is subject to the policy;
//!   * a policy with both `USING` (reads/deletes) and `WITH CHECK` (writes), so a
//!     row can neither be *seen* nor *written* outside the active tenant;
//!   * coverage of **every** table that has a `NOT NULL tenant_id` column, not
//!     just the original seven — discovered dynamically so it can't drift.
//!
//! Tables whose `tenant_id` is nullable (identity/global: `app_user`, `audit_log`,
//! `membership`, `user_role`) or absent (`tenant`, `role`, `role_permission`,
//! `refresh_token`) are intentionally excluded — they back login, RBAC, and the
//! platform plane and must remain readable with no tenant context.
//!
//! Contract with the app: [`api::db::RequestDb`] runs each request in a
//! transaction and sets `app.tenant_id` via `set_config(_, _, true)` (`SET
//! LOCAL`). When it is unset (platform staff at Acre HQ, login, background jobs)
//! the policy's `IS NULL` branch allows all rows — the intentional cross-tenant
//! plane.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Enable + force RLS and (re)create the isolation policy on every base table in
/// `public` that has a `NOT NULL tenant_id` column.
const UP_SQL: &str = r#"
DO $$
DECLARE
    t text;
    pred text := 'current_setting(''app.tenant_id'', true) IS NULL OR tenant_id::text = current_setting(''app.tenant_id'', true)';
BEGIN
    FOR t IN
        SELECT c.table_name
        FROM information_schema.columns c
        JOIN information_schema.tables tb
          ON tb.table_schema = c.table_schema
         AND tb.table_name = c.table_name
        WHERE c.table_schema = 'public'
          AND c.column_name = 'tenant_id'
          AND c.is_nullable = 'NO'
          AND tb.table_type = 'BASE TABLE'
    LOOP
        EXECUTE format('ALTER TABLE %I ENABLE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('DROP POLICY IF EXISTS %I ON %I', t || '_tenant_isolation', t);
        EXECUTE format(
            'CREATE POLICY %I ON %I USING (%s) WITH CHECK (%s)',
            t || '_tenant_isolation', t, pred, pred
        );
    END LOOP;
END $$;
"#;

/// Revert to the un-enforced state: drop the policy, unforce, and disable RLS on
/// every tenant-owned table.
const DOWN_SQL: &str = r#"
DO $$
DECLARE
    t text;
BEGIN
    FOR t IN
        SELECT c.table_name
        FROM information_schema.columns c
        JOIN information_schema.tables tb
          ON tb.table_schema = c.table_schema
         AND tb.table_name = c.table_name
        WHERE c.table_schema = 'public'
          AND c.column_name = 'tenant_id'
          AND c.is_nullable = 'NO'
          AND tb.table_type = 'BASE TABLE'
    LOOP
        EXECUTE format('DROP POLICY IF EXISTS %I ON %I', t || '_tenant_isolation', t);
        EXECUTE format('ALTER TABLE %I NO FORCE ROW LEVEL SECURITY', t);
        EXECUTE format('ALTER TABLE %I DISABLE ROW LEVEL SECURITY', t);
    END LOOP;
END $$;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP_SQL).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(DOWN_SQL)
            .await?;
        Ok(())
    }
}
