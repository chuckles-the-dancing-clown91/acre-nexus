//! Make the tenant-isolation RLS policy robust to an **empty-string**
//! `app.tenant_id`, not just an unset one.
//!
//! Postgres quirk: a *custom* GUC (`app.tenant_id`) set with `SET LOCAL` reverts
//! to the empty string `''` — **not** `NULL` — when the transaction ends. So on
//! any pooled connection that has *ever* served a tenant-scoped request,
//! `current_setting('app.tenant_id', true)` returns `''` on the next
//! transaction, even before [`api::db::RequestDb`] sets it again.
//!
//! Migration `000015`'s predicate keyed the cross-tenant plane on
//! `current_setting(...) IS NULL`. That branch never matches `''`, so a
//! platform-plane request (staff at Acre HQ, login, background jobs — which set
//! *no* tenant) landing on a reused connection would be denied **all** tenant
//! rows once the app connects as a role actually subject to RLS (the intended
//! production shape). This recreates the policy with
//! `NULLIF(current_setting('app.tenant_id', true), '')`, so both an unset and a
//! reset-to-empty GUC resolve to the "no tenant context → all rows" branch,
//! while a real tenant id still scopes to that tenant.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Recreate the isolation policy on every base table with a `NOT NULL tenant_id`
/// column, treating an empty-string GUC the same as an unset one.
const UP_SQL: &str = r#"
DO $$
DECLARE
    t text;
    pred text := 'NULLIF(current_setting(''app.tenant_id'', true), '''') IS NULL OR tenant_id::text = NULLIF(current_setting(''app.tenant_id'', true), '''')';
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
        EXECUTE format(
            'CREATE POLICY %I ON %I USING (%s) WITH CHECK (%s)',
            t || '_tenant_isolation', t, pred, pred
        );
    END LOOP;
END $$;
"#;

/// Restore migration `000015`'s `IS NULL`-only predicate.
const DOWN_SQL: &str = r#"
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
        EXECUTE format('DROP POLICY IF EXISTS %I ON %I', t || '_tenant_isolation', t);
        EXECUTE format(
            'CREATE POLICY %I ON %I USING (%s) WITH CHECK (%s)',
            t || '_tenant_isolation', t, pred, pred
        );
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
