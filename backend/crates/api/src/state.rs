//! Shared application state, managed by Rocket and accessible from guards/handlers.

use crate::config::Config;
use sea_orm::{
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbErr, Statement,
    TransactionTrait,
};
use uuid::Uuid;

/// One connection (pool) per domain database, plus runtime config.
///
/// - `user_db` — `acre_user`: identity, auth, RBAC, tenancy, themes, plus the
///   cross-cutting `audit_log` and `background_job` tables.
/// - `property_db` — `acre_property`: properties and all of their data, rentals,
///   maintenance, title and financing.
/// - `client_db` — `acre_client`: counterparties, their notes, and applications.
#[derive(Clone)]
pub struct AppState {
    pub user_db: DatabaseConnection,
    pub property_db: DatabaseConnection,
    pub client_db: DatabaseConnection,
    pub config: Config,
}

impl AppState {
    /// Begin a transaction on `conn` clamped to `tenant_id` via
    /// `SET LOCAL app.tenant_id`, so Postgres row-level-security policies enforce
    /// tenant isolation for its duration (defence in depth beneath the
    /// application-layer `tenant_id` filters).
    ///
    /// Use for tenant-scoped reads/writes and **commit on success** — an
    /// uncommitted transaction rolls back on drop. `set_config(..., true)` makes
    /// the setting transaction-local, so it never leaks across pooled
    /// connections. Do **not** use this for cross-tenant work (the scheduler, the
    /// platform-admin tenant registry): those intentionally run unclamped.
    pub async fn tenant_tx(
        conn: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<DatabaseTransaction, DbErr> {
        let txn = conn.begin().await?;
        txn.execute(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            "SELECT set_config('app.tenant_id', $1, true)",
            [tenant_id.to_string().into()],
        ))
        .await?;
        Ok(txn)
    }
}
