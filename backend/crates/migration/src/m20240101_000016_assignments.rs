//! **Staff assignments** — attach people (property managers, landlords,
//! maintenance, leasing agents, back-office) to a specific property or legal
//! entity (LLC). An assignment is both a directory relationship and an access
//! grant: the API pairs each row with a scoped `user_role` grant so the person
//! can actually act on that property/LLC (see `routes/assignments`).
//!
//! The table is tenant-owned, so it gets the same enforced RLS as every other
//! tenant table (ENABLE + FORCE + USING/WITH CHECK on `app.tenant_id`), matching
//! `m20240101_000015_rls_enforce`. Because that dynamic migration already ran,
//! this migration must enable RLS on the new table itself.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

fn col(name: &str) -> ColumnDef {
    ColumnDef::new(Alias::new(name)).take()
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

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("assignment"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    // `property` | `entity` (LLC)
                    .col(col("subject_type").string().not_null())
                    .col(col("subject_id").uuid().not_null())
                    .col(col("user_id").uuid().not_null())
                    // tenant role key: property_manager | landlord | maintenance | …
                    .col(col("relationship").string().not_null())
                    // the role.id actually granted for this scope (nullable)
                    .col(col("role_id").uuid().null())
                    .col(col("is_primary").boolean().not_null().default(false))
                    .col(col("title").string().null())
                    .col(col("notes").text().null())
                    .col(col("assigned_by").uuid().null())
                    .col(
                        col("created_at")
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        col("updated_at")
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        index(manager, "assignment", "tenant_id").await?;
        index(manager, "assignment", "subject_id").await?;
        index(manager, "assignment", "user_id").await?;

        // One row per (tenant, subject, user, relationship).
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("uq_assignment_subject_user_rel")
                    .table(Alias::new("assignment"))
                    .col(Alias::new("tenant_id"))
                    .col(Alias::new("subject_type"))
                    .col(Alias::new("subject_id"))
                    .col(Alias::new("user_id"))
                    .col(Alias::new("relationship"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Enforced RLS on the new tenant-owned table (matches migration 000015).
        let sql = format!(
            "ALTER TABLE assignment ENABLE ROW LEVEL SECURITY; \
             ALTER TABLE assignment FORCE ROW LEVEL SECURITY; \
             DROP POLICY IF EXISTS assignment_tenant_isolation ON assignment; \
             CREATE POLICY assignment_tenant_isolation ON assignment \
               USING ({RLS_PRED}) WITH CHECK ({RLS_PRED});"
        );
        manager.get_connection().execute_unprepared(&sql).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP POLICY IF EXISTS assignment_tenant_isolation ON assignment;")
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("assignment"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
