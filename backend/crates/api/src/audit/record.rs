//! The **domain-event** writer.
//!
//! [`record`] persists one semantic audit entry — who did what, to which target,
//! in which workspace, with optional structured detail. It is best-effort: any
//! failure is logged and swallowed so an audit write can never block or fail the
//! calling request. Request-context columns are left `NULL` here; those are
//! filled only by the per-request fairing ([`super::request_log`]).

use sea_orm::{ActiveModelTrait, DatabaseConnection, NotSet, Set};
use uuid::Uuid;

/// Record a domain audit entry. Best-effort: errors are logged, not propagated.
pub async fn record(
    db: &DatabaseConnection,
    actor: Option<Uuid>,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<String>,
    tenant_id: Option<Uuid>,
    metadata: Option<serde_json::Value>,
) {
    let entry = entity::audit_log::ActiveModel {
        id: Set(Uuid::new_v4()),
        actor_user_id: Set(actor),
        action: Set(action.to_string()),
        target_type: Set(target_type.map(|s| s.to_string())),
        target_id: Set(target_id),
        tenant_id: Set(tenant_id),
        metadata: Set(metadata),
        // Request-context columns are only meaningful for fairing entries.
        method: NotSet,
        path: NotSet,
        status_code: NotSet,
        request_id: NotSet,
        ip: NotSet,
        duration_ms: NotSet,
        // A domain event with no actor is a system/anonymous action (e.g. a
        // public application submission); otherwise it was a signed-in user.
        principal_kind: Set(Some(
            if actor.is_some() { "user" } else { "public" }.to_string(),
        )),
        created_at: Set(chrono::Utc::now().into()),
    };
    if let Err(e) = entry.insert(db).await {
        tracing::error!("audit write failed for action '{action}': {e}");
    }
}
