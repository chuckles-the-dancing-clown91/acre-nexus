//! The built-in Maintenance role no longer reads leases: field crew work
//! tickets, schedules and their own time, not tenants. Removes the grant from
//! the system role (custom roles a workspace made from it are left alone).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "DELETE FROM role_permission WHERE permission = 'lease:read' \
                 AND role_id IN (SELECT id FROM role WHERE key = 'maintenance' AND is_system = true);",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO role_permission (role_id, permission) \
                 SELECT id, 'lease:read' FROM role WHERE key = 'maintenance' AND is_system = true \
                 ON CONFLICT DO NOTHING;",
            )
            .await?;
        Ok(())
    }
}
