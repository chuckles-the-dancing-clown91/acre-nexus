//! **Audit trail** (Vantedge roadmap area 1): who changed what on which property.
//!
//! * `audit_log.property_id` — the property an event is about (a unit, asset,
//!   ticket, lease or listing event carries its property), so a property's
//!   history is one indexed query.
//! * `audit_log.support` — a Vantedge employee made the change while acting on
//!   the workspace; the customer sees that in their own history.
//! * Indexes for the history, target and filtered-list queries.
//!
//! `audit_log` stays outside row-level security (its tenant id is nullable, it
//! backs the platform plane); every query on it filters `tenant_id` itself.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE audit_log ADD COLUMN IF NOT EXISTS property_id uuid NULL; \
                 ALTER TABLE audit_log ADD COLUMN IF NOT EXISTS support boolean NOT NULL DEFAULT false; \
                 CREATE INDEX IF NOT EXISTS idx_audit_log_property_created \
                   ON audit_log (property_id, created_at DESC) WHERE property_id IS NOT NULL; \
                 CREATE INDEX IF NOT EXISTS idx_audit_log_target \
                   ON audit_log (target_type, target_id, created_at DESC) WHERE target_type IS NOT NULL; \
                 CREATE INDEX IF NOT EXISTS idx_audit_log_tenant_events \
                   ON audit_log (tenant_id, created_at DESC) WHERE action <> 'http.request';",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DROP INDEX IF EXISTS idx_audit_log_tenant_events; \
                 DROP INDEX IF EXISTS idx_audit_log_target; \
                 DROP INDEX IF EXISTS idx_audit_log_property_created; \
                 ALTER TABLE audit_log DROP COLUMN IF EXISTS support; \
                 ALTER TABLE audit_log DROP COLUMN IF EXISTS property_id;",
            )
            .await?;
        Ok(())
    }
}
