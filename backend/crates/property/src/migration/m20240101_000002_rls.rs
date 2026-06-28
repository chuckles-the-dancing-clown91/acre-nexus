//! Row-level-security for the tenant-scoped tables in the **property** database.
//! See the user-domain RLS migration for the strict-enforcement rationale.

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240101_000002_property_rls"
    }
}

/// Tenant-scoped tables hosted in the `acre_property` database.
const TENANT_SCOPED: &[&str] = &["llc", "property", "listing"];

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
