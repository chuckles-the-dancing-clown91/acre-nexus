//! The **per-request** writer used by the [`super::fairing`].
//!
//! Persists one `audit_log` row describing an HTTP request/response, populating
//! the request-context columns the domain-event writer leaves `NULL`. Like
//! [`super::record`] it is best-effort — a failed insert is logged, never
//! propagated.

use super::actions;
use super::actor::ResolvedActor;
use sea_orm::{ActiveModelTrait, DatabaseConnection, NotSet, Set};
use uuid::Uuid;

/// Everything the fairing knows about a finished request.
pub struct RequestRecord {
    pub actor: ResolvedActor,
    pub method: String,
    pub path: String,
    pub status_code: i32,
    pub request_id: Uuid,
    pub ip: Option<String>,
    pub duration_ms: i64,
}

/// Write a request entry. Best-effort: errors are logged, not propagated.
pub async fn write(db: &DatabaseConnection, rec: RequestRecord) {
    let entry = entity::audit_log::ActiveModel {
        id: Set(Uuid::new_v4()),
        actor_user_id: Set(rec.actor.user_id),
        action: Set(actions::HTTP_REQUEST.to_string()),
        target_type: NotSet,
        target_id: NotSet,
        tenant_id: Set(rec.actor.tenant_id),
        metadata: NotSet,
        method: Set(Some(rec.method)),
        path: Set(Some(rec.path)),
        status_code: Set(Some(rec.status_code)),
        request_id: Set(Some(rec.request_id)),
        ip: Set(rec.ip),
        duration_ms: Set(Some(rec.duration_ms)),
        principal_kind: Set(Some(rec.actor.kind.to_string())),
        created_at: Set(chrono::Utc::now().into()),
    };
    if let Err(e) = entry.insert(db).await {
        tracing::error!("audit request-log write failed: {e}");
    }
}
