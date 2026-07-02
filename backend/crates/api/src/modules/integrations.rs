//! **Integrations** module — the cross-cutting integration substrate
//! (roadmap Phase 1, issues #15–#19): encrypted credential storage, the
//! document service, transactional notifications, and inbound webhook
//! ingestion. On by default — this is foundational plumbing every tenant
//! needs, not an optional add-on.
//!
//! It owns the notification job kinds (`auto_email` moved here from `leasing`
//! — reminders, renewal notices, and statutory notices all send mail and have
//! nothing to do with leasing; the `{template, to}` payload contract is
//! unchanged), the verified-webhook event kind, and document retention.

use super::{JobContext, JobOutcome, ModuleManifest, PlatformModule};
use crate::rbac::Permission;
use crate::routes::{documents, integrations};
use crate::storage::ObjectStore;
use entity::prelude::Document;
use rocket::Route;
use rocket_okapi::okapi::openapi3::OpenApi;
use rocket_okapi::openapi_get_routes_spec;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait, QueryFilter};
use serde_json::json;
use uuid::Uuid;

pub struct IntegrationsModule;

#[rocket::async_trait]
impl PlatformModule for IntegrationsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest {
            key: "integrations",
            name: "Integrations",
            description: "Credential vault, document storage, notifications (email/SMS), \
                 and inbound webhooks — the substrate external integrations ride on.",
            permissions: &[
                Permission::IntegrationsManage,
                Permission::DocumentRead,
                Permission::DocumentManage,
            ],
            job_kinds: &[
                "auto_email",
                "auto_sms",
                "webhook_event",
                "document_retention",
            ],
            default_enabled: true,
            preview: false,
        }
    }

    fn api(&self) -> (Vec<Route>, OpenApi) {
        openapi_get_routes_spec![
            // secrets: write-only credential vault
            integrations::list_secrets::list_secrets,
            integrations::set_secret::set_secret,
            integrations::delete_secret::delete_secret,
            // notification send history
            integrations::list_notifications::list_notifications,
            // inbound webhooks (signature-verified, queue-backed)
            integrations::webhook::receive,
            // document service
            documents::upload::upload,
            documents::list::list,
            documents::download::download,
            documents::delete::delete,
            // local object-store blob endpoints (dev/CI backend)
            documents::blob::put_blob,
            documents::blob::get_blob,
        ]
    }

    async fn handle_job(&self, ctx: &JobContext<'_>) -> Option<JobOutcome> {
        match ctx.job.kind.as_str() {
            "auto_email" | "auto_sms" => Some(crate::notify::handle_job(ctx.db, ctx.job).await),
            "document_retention" => Some(retention(ctx.db, ctx.job).await),
            // Phase 1 has no event consumers yet: a verified event is recorded
            // as processed; providers landing in later phases (#35/#36/#8)
            // dispatch on `payload.provider` here.
            "webhook_event" => {
                let provider = ctx
                    .job
                    .payload
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                Some(JobOutcome::completed(json!({
                    "provider": provider,
                    "processed": true,
                })))
            }
            _ => None,
        }
    }
}

/// Advance one `document_retention` job: delete the document once its
/// retention window has passed (idempotent — a missing document is done), or
/// park until the deadline if it moved.
async fn retention(db: &DatabaseConnection, job: &entity::background_job::Model) -> JobOutcome {
    let Some(doc_id) = job
        .payload
        .get("document_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return JobOutcome::failed("retention payload missing document_id");
    };

    let doc = match Document::find_by_id(doc_id)
        .filter(entity::document::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
    {
        Ok(Some(d)) => d,
        Ok(None) => return JobOutcome::completed(json!({ "already_gone": true })),
        Err(e) => {
            return JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            )
        }
    };

    let now = chrono::Utc::now();
    match doc.retention_expires_at {
        // Retention was extended/cleared after this job was scheduled.
        None => JobOutcome::completed(json!({ "retention_cleared": true })),
        Some(expiry) if expiry > now => {
            let delay = (expiry.with_timezone(&chrono::Utc) - now)
                .num_seconds()
                .max(60);
            JobOutcome::reschedule("pending", delay)
        }
        Some(_) => {
            let store = match ObjectStore::from_env() {
                Ok(s) => s,
                Err(e) => {
                    return JobOutcome::retry(
                        crate::providers::backoff(job.attempts),
                        e.to_string(),
                    )
                }
            };
            if let Err(e) = store.delete(&doc.storage_key).await {
                return JobOutcome::retry(crate::providers::backoff(job.attempts), e.to_string());
            }
            let filename = doc.filename.clone();
            if let Err(e) = doc.delete(db).await {
                return JobOutcome::retry(
                    crate::providers::backoff(job.attempts),
                    format!("db error: {e}"),
                );
            }
            crate::audit::record(
                db,
                None,
                crate::audit::actions::DOCUMENT_DELETE,
                Some("document"),
                Some(doc_id.to_string()),
                Some(job.tenant_id),
                Some(json!({ "filename": filename, "reason": "retention_expired" })),
            )
            .await;
            JobOutcome::completed(json!({ "deleted": true, "reason": "retention_expired" }))
        }
    }
}
