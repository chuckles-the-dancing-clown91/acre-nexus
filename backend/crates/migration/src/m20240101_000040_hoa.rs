//! **HOA / association management** (issue #13, Beyond-GA vertical):
//!
//! * `hoa_association` — the governing body for a community.
//! * `hoa_member` — a homeowner / unit in the association.
//! * `hoa_assessment` — a dues charge (recurring or special) to a member.
//! * `hoa_violation` — a CC&R violation and its enforcement lifecycle.
//! * `hoa_arc_request` — an architectural-review request and its decision.
//!
//! Tenant-owned; RLS is enforced with the empty-string-safe predicate (see
//! migration `000038`).

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

const RLS_PRED: &str = "NULLIF(current_setting('app.tenant_id', true), '') IS NULL \
     OR tenant_id::text = NULLIF(current_setting('app.tenant_id', true), '')";

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
                    .table(Alias::new("hoa_association"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("property_id").uuid().null())
                    .col(col("dues_cents").big_integer().not_null().default(0))
                    .col(col("dues_frequency").string().not_null().default("monthly"))
                    .col(col("status").string().not_null().default("active"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "hoa_association", "tenant_id").await?;
        enforce_rls(manager, "hoa_association").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("hoa_member"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("association_id").uuid().not_null())
                    .col(col("name").string().not_null())
                    .col(col("unit_label").string().null())
                    .col(col("email").string().null())
                    .col(col("phone").string().null())
                    .col(col("status").string().not_null().default("active"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "hoa_member", "tenant_id").await?;
        index(manager, "hoa_member", "association_id").await?;
        enforce_rls(manager, "hoa_member").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("hoa_assessment"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("association_id").uuid().not_null())
                    .col(col("member_id").uuid().not_null())
                    .col(col("description").string().not_null())
                    .col(col("amount_cents").big_integer().not_null().default(0))
                    .col(col("period").string().null())
                    .col(col("due_date").string().null())
                    .col(col("status").string().not_null().default("due"))
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "hoa_assessment", "tenant_id").await?;
        index(manager, "hoa_assessment", "member_id").await?;
        enforce_rls(manager, "hoa_assessment").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("hoa_violation"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("association_id").uuid().not_null())
                    .col(col("member_id").uuid().not_null())
                    .col(col("kind").string().not_null())
                    .col(col("description").text().not_null().default(""))
                    .col(col("status").string().not_null().default("open"))
                    .col(col("fine_cents").big_integer().not_null().default(0))
                    .col(col("resolved_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "hoa_violation", "tenant_id").await?;
        index(manager, "hoa_violation", "member_id").await?;
        enforce_rls(manager, "hoa_violation").await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("hoa_arc_request"))
                    .if_not_exists()
                    .col(col("id").uuid().not_null().primary_key())
                    .col(col("tenant_id").uuid().not_null())
                    .col(col("association_id").uuid().not_null())
                    .col(col("member_id").uuid().not_null())
                    .col(col("title").string().not_null())
                    .col(col("description").text().not_null().default(""))
                    .col(col("status").string().not_null().default("submitted"))
                    .col(col("decision_note").text().null())
                    .col(col("decided_by").uuid().null())
                    .col(col("decided_at").timestamp_with_time_zone().null())
                    .col(ts("created_at"))
                    .to_owned(),
            )
            .await?;
        index(manager, "hoa_arc_request", "tenant_id").await?;
        index(manager, "hoa_arc_request", "member_id").await?;
        enforce_rls(manager, "hoa_arc_request").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for table in [
            "hoa_arc_request",
            "hoa_violation",
            "hoa_assessment",
            "hoa_member",
            "hoa_association",
        ] {
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
