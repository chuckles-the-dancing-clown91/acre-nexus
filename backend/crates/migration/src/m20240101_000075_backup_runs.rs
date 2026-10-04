//! **Backups you can see**: every nightly backup and every restore drill
//! records a row here (written by `backend/deploy/backup.sh` and
//! `restore-drill.sh`), so the go-live page can say when the last good one
//! ran and how long a restore took. Platform-wide: no tenant, no RLS.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r#"
CREATE TABLE IF NOT EXISTS backup_run (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    kind text NOT NULL CHECK (kind IN ('backup', 'restore_drill')),
    started_at timestamptz NOT NULL,
    finished_at timestamptz NULL,
    ok boolean NOT NULL DEFAULT false,
    bytes bigint NULL,
    location text NULL,
    detail text NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS ix_backup_run_kind_started ON backup_run (kind, started_at DESC);
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS backup_run;")
            .await?;
        Ok(())
    }
}
