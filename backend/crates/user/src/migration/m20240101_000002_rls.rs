//! Row-level-security for the tenant-scoped tables in the **user** database.
//!
//! Strict enforcement (the platform connects at runtime as a non-owner `_app`
//! role): we `ENABLE` **and** `FORCE ROW LEVEL SECURITY` so even the table owner
//! is subject to the policy, then add a tenant-isolation policy keyed on the
//! `app.tenant_id` session variable set per request via `SET LOCAL`.
//!
//! The policy is permissive when `app.tenant_id` is unset (`IS NULL`), so
//! migrations, seeding and the cross-tenant background scheduler — none of which
//! set the variable — keep working.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000002_user_rls"
    }
}

/// Tenant-scoped tables hosted in the `acre_user` database.
const TENANT_SCOPED: &[&str] = &["api_token", "theme", "background_job"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for t in TENANT_SCOPED {
            let sql = format!(
                "ALTER TABLE {t} ENABLE ROW LEVEL SECURITY; \
                 ALTER TABLE {t} FORCE ROW LEVEL SECURITY; \
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
                 ALTER TABLE {t} NO FORCE ROW LEVEL SECURITY; \
                 ALTER TABLE {t} DISABLE ROW LEVEL SECURITY;"
            );
            db.execute_unprepared(&sql).await?;
        }
        Ok(())
    }
}
