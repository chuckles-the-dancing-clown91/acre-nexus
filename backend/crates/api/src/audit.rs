//! Best-effort audit logging for security-relevant actions.
//!
//! [`record`] writes an [`entity::audit_log`] row and never fails the calling
//! request — an audit write failure is logged and swallowed so it can't block
//! the underlying operation. Sensitive handlers (PII reveal, role/user changes)
//! call this; the dashboard surfaces the trail via `GET /admin/audit`.

use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

/// Record an audit entry. Best-effort: errors are logged, not propagated.
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
        created_at: Set(chrono::Utc::now().into()),
    };
    if let Err(e) = entry.insert(db).await {
        tracing::error!("audit write failed for action '{action}': {e}");
    }
}
