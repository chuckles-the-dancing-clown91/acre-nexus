//! Row-level-security scaffolding — *defence in depth* for tenant isolation.
//!
//! The application already filters every query by `tenant_id`. These policies add
//! a second wall at the database: a tenant-scoped policy keyed on the
//! `app.tenant_id` session variable that the API can set per request.
//!
//! Note: a table's *owner* bypasses RLS unless `FORCE ROW LEVEL SECURITY` is set.
//! For production, connect the API as a dedicated non-owner role and set
//! `SET app.tenant_id = '<uuid>'` per transaction to make these policies bite.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const TENANT_SCOPED: &[&str] = &[
    "llc",
    "property",
    "listing",
    "application",
    "api_token",
    "theme",
    "background_job",
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
        Ok(())
    }
}
