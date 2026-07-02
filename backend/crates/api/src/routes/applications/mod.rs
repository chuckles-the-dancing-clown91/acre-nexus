//! Landlord/PM application management (tenant-scoped, RBAC-gated).

pub mod convert;
pub mod dto;
pub mod list;
pub mod reuse;
pub mod update_status;
pub mod workflow;

use crate::error::{ApiError, ApiResult};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use serde_json::json;
use uuid::Uuid;

/// Apply a validated status transition to an application: checks the
/// [`crate::app_workflow`] state machine, updates `status`, records an immutable
/// `application_event`, audits, and fires the approval side-effect. Shared by the
/// `PATCH /applications/<id>` and `POST /applications/<id>/advance` handlers.
pub(crate) async fn apply_transition(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    actor: Uuid,
    app: entity::application::Model,
    to_status: &str,
    note: Option<String>,
) -> ApiResult<entity::application::Model> {
    let from = app.status.clone();
    if !crate::app_workflow::is_known_stage(to_status) {
        return Err(ApiError::BadRequest(format!(
            "unknown application status: {to_status}"
        )));
    }
    if !crate::app_workflow::is_valid_transition(&from, to_status) {
        return Err(ApiError::BadRequest(format!(
            "cannot move an application from '{from}' to '{to_status}'"
        )));
    }

    let mut am: entity::application::ActiveModel = app.into();
    am.status = Set(to_status.to_string());
    let saved = am.update(db).await?;

    entity::application_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        application_id: Set(saved.id),
        from_status: Set(Some(from.clone())),
        to_status: Set(to_status.to_string()),
        note: Set(note.clone()),
        actor_user_id: Set(Some(actor)),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;

    crate::audit::record(
        db,
        Some(actor),
        crate::audit::actions::APPLICATION_ADVANCE,
        Some("application"),
        Some(saved.id.to_string()),
        Some(tenant_id),
        Some(json!({ "from": from, "to": to_status, "note": note })),
    )
    .await;

    // Approving an application still kicks off the automated welcome email.
    // The owner/trigger fields give the notification engine its idempotency
    // key, so re-approving (or a retried job) can't double-send.
    if to_status == "Approved" {
        let _ = crate::scheduler::enqueue(
            db,
            tenant_id,
            "auto_email",
            json!({
                "template": "application_approved",
                "to": saved.email,
                "owner_type": "application",
                "owner_id": saved.id,
                "trigger": "approved",
            }),
            0,
        )
        .await;
    }

    Ok(saved)
}
