//! **Transactional notifications** — real email + SMS behind the `auto_email` /
//! `auto_sms` job kinds (roadmap issue #18).
//!
//! `auto_email` predates this module: the apply funnel has always enqueued
//! `{ "template": …, "to": … }` jobs, but the old handler was a
//! fire-and-complete stub. This module makes the kind real **without changing
//! that payload contract**: templates render through the same `{placeholder}`
//! engine as lease documents ([`crate::leasedoc::interpolate`]), tenant
//! overrides live on `theme.notification_templates` (sibling to
//! `legal_templates`), delivery rides the [`crate::providers`] trait
//! (simulated by default, a real ESP plugs in via #62), every send is
//! persisted to `notification` and audited, and an idempotency key keeps a
//! retried job or duplicate trigger from double-sending.

use crate::leasedoc::interpolate;
use crate::modules::JobOutcome;
use crate::providers::{self, err, Provider, ProviderCtx, ProviderError};
use chrono::Utc;
use entity::prelude::{Notification, Theme};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// One default template: rendered bodies per channel, overridable per tenant
/// via `theme.notification_templates`.
struct DefaultTemplate {
    key: &'static str,
    subject: &'static str,
    body: &'static str,
    sms: &'static str,
}

/// Platform default templates. A tenant override with the same key (a plain
/// body string, or `{ "subject": …, "body": …, "sms": … }`) wins field by
/// field.
const DEFAULT_TEMPLATES: &[DefaultTemplate] = &[
    DefaultTemplate {
        key: "application_approved",
        subject: "Your application with {company} has been approved",
        body: "Hi {recipient},\n\nGreat news — your rental application with {company} has been \
               approved. We'll be in touch shortly with next steps.\n\n— {company}",
        sms: "{company}: good news — your rental application has been approved. \
              We'll text you next steps shortly.",
    },
    DefaultTemplate {
        key: "application_received",
        subject: "We received your application",
        body: "Hi {recipient},\n\nThanks for applying with {company}. Your application is in \
               review and we'll notify you as soon as there's a decision.\n\n— {company}",
        sms: "{company}: thanks — we received your application and will be in touch soon.",
    },
];

/// A rendered, ready-to-send message.
struct Rendered {
    subject: Option<String>,
    body: String,
}

/// Resolve + render `template_key` for `channel`, layering the tenant override
/// (if any) over the platform default. `None` when the key is unknown to both.
fn render(
    overrides: &serde_json::Value,
    channel: &str,
    template_key: &str,
    vars: &HashMap<&str, String>,
) -> Option<Rendered> {
    let default = DEFAULT_TEMPLATES.iter().find(|t| t.key == template_key);
    let over = overrides.get(template_key);

    let str_field = |name: &str| -> Option<String> {
        over.and_then(|o| o.get(name))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    // A bare-string override is an email/SMS body in one.
    let over_plain = over.and_then(|o| o.as_str()).map(str::to_string);

    let body_template = match channel {
        "sms" => str_field("sms")
            .or_else(|| over_plain.clone())
            .or(default.map(|d| d.sms.to_string()))?,
        _ => str_field("body")
            .or_else(|| over_plain.clone())
            .or(default.map(|d| d.body.to_string()))?,
    };
    let subject = match channel {
        "sms" => None,
        _ => Some(
            str_field("subject")
                .or(default.map(|d| d.subject.to_string()))
                .unwrap_or_else(|| "Notification from {company}".to_string()),
        ),
    };

    Some(Rendered {
        subject: subject.map(|s| interpolate(&s, vars)),
        body: interpolate(&body_template, vars),
    })
}

// ---------------------------------------------------------------------------
// Delivery providers (#16 trait; simulated until #62 lands a real ESP)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct MessageRequest {
    pub to: String,
    pub subject: Option<String>,
    pub body: String,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub provider_message_id: String,
}

/// Email delivery. `call` is the slot the real ESP connector (#62) fills in;
/// until then live mode fails loudly rather than pretending to send.
pub struct EmailProvider;

#[async_trait::async_trait]
impl Provider for EmailProvider {
    type Request = MessageRequest;
    type Response = MessageResponse;

    fn key(&self) -> &'static str {
        "email"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        _req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        Err(err(
            "no live email provider is wired up yet (ESP connector lands with issue #62); \
             unset LIVE_PROVIDERS to use the simulated sender",
        ))
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        tracing::info!(to = %req.to, subject = ?req.subject, "simulated email sent");
        Ok(MessageResponse {
            provider_message_id: format!("sim-email-{}", Uuid::new_v4().simple()),
        })
    }
}

/// SMS delivery, same shape as email.
pub struct SmsProvider;

#[async_trait::async_trait]
impl Provider for SmsProvider {
    type Request = MessageRequest;
    type Response = MessageResponse;

    fn key(&self) -> &'static str {
        "sms"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        _req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        Err(err(
            "no live SMS provider is wired up yet; unset LIVE_PROVIDERS to use the \
             simulated sender",
        ))
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        tracing::info!(to = %req.to, "simulated SMS sent");
        Ok(MessageResponse {
            provider_message_id: format!("sim-sms-{}", Uuid::new_v4().simple()),
        })
    }
}

// ---------------------------------------------------------------------------
// Job handling
// ---------------------------------------------------------------------------

/// The natural de-duplication key for a send, when the payload carries enough
/// context to build one (`owner_type` + `owner_id`, plus an optional
/// `trigger`). Legacy payloads without owner context fall back to per-job
/// dedup via `notification.background_job_id`.
fn idempotency_key(payload: &serde_json::Value, template: &str, channel: &str) -> Option<String> {
    let owner_type = payload.get("owner_type")?.as_str()?;
    let owner_id = payload.get("owner_id")?.as_str()?;
    let trigger = payload
        .get("trigger")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    Some(format!(
        "{channel}:{template}:{owner_type}:{owner_id}:{trigger}"
    ))
}

/// Advance one `auto_email` / `auto_sms` job: render, send (simulated or
/// live), persist the `notification` row, audit, and map provider errors onto
/// the retry budget. Called from the integrations module's `handle_job`.
pub async fn handle_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let channel = match job.kind.as_str() {
        "auto_sms" => "sms",
        _ => "email",
    };
    let Some(template) = job.payload.get("template").and_then(|v| v.as_str()) else {
        return JobOutcome::failed("notification payload missing 'template'");
    };
    let Some(to) = job.payload.get("to").and_then(|v| v.as_str()) else {
        return JobOutcome::failed("notification payload missing 'to'");
    };

    // Idempotency: has this natural trigger already sent (or is it in flight on
    // another job)?
    let idem = idempotency_key(&job.payload, template, channel);
    if let Some(key) = &idem {
        let existing = Notification::find()
            .filter(entity::notification::Column::TenantId.eq(job.tenant_id))
            .filter(entity::notification::Column::IdempotencyKey.eq(key.clone()))
            .one(db)
            .await;
        if let Ok(Some(n)) = existing {
            if n.background_job_id != Some(job.id) && n.status != "failed" {
                return JobOutcome::completed(json!({
                    "deduped": true,
                    "notification_id": n.id,
                }));
            }
        }
    }

    // Tenant theme: company name for template vars + per-tenant overrides.
    let theme = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(job.tenant_id))
        .one(db)
        .await
        .ok()
        .flatten();
    let company = theme
        .as_ref()
        .map(|t| t.company_name.clone())
        .unwrap_or_else(|| "Acre Nexus".to_string());
    let overrides = theme
        .map(|t| t.notification_templates)
        .unwrap_or_else(|| json!({}));

    // Interpolation vars: built-ins + any string vars the enqueuer provided.
    let mut pairs: Vec<(String, String)> = vec![
        ("recipient".into(), to.to_string()),
        ("company".into(), company),
        ("template".into(), template.to_string()),
    ];
    if let Some(extra) = job.payload.get("vars").and_then(|v| v.as_object()) {
        for (k, v) in extra {
            let val = v
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string());
            pairs.push((k.clone(), val));
        }
    }
    let vars: HashMap<&str, String> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();

    let Some(rendered) = render(&overrides, channel, template, &vars) else {
        return JobOutcome::failed(format!(
            "unknown notification template '{template}' (no platform default, no tenant override)"
        ));
    };

    // Ensure the notification row exists (find by job first so a retry updates
    // rather than duplicates).
    let row = match ensure_row(db, job, channel, template, to, idem.as_deref()).await {
        Ok(r) => r,
        Err(e) => return JobOutcome::retry(providers::backoff(job.attempts), e.to_string()),
    };

    // Deliver through the provider framework (#16).
    let ctx = ProviderCtx::new(db, job.tenant_id);
    let req = MessageRequest {
        to: to.to_string(),
        subject: rendered.subject.clone(),
        body: rendered.body.clone(),
    };
    let outcome = match channel {
        "sms" => providers::run(&SmsProvider, &ctx, job, &req).await,
        _ => providers::run(&EmailProvider, &ctx, job, &req).await,
    };

    match outcome {
        Ok(resp) => {
            let notification_id = row.id;
            let mut am: entity::notification::ActiveModel = row.into();
            am.status = Set("sent".into());
            am.provider_message_id = Set(Some(resp.provider_message_id.clone()));
            am.subject = Set(rendered.subject);
            am.body = Set(Some(rendered.body));
            am.last_error = Set(None);
            am.updated_at = Set(Utc::now().into());
            if let Err(e) = am.update(db).await {
                tracing::error!("failed to persist sent notification: {e}");
            }

            // Audit the send: template + channel + status, never the rendered
            // body (it may carry PII).
            crate::audit::record(
                db,
                None,
                crate::audit::actions::NOTIFICATION_SEND,
                Some("notification"),
                Some(notification_id.to_string()),
                Some(job.tenant_id),
                Some(json!({
                    "template": template,
                    "channel": channel,
                    "status": "sent",
                })),
            )
            .await;

            JobOutcome::completed(json!({
                "sent": true,
                "channel": channel,
                "template": template,
                "notification_id": notification_id,
                "provider_message_id": resp.provider_message_id,
                "sent_at": Utc::now().to_rfc3339(),
            }))
        }
        Err(job_outcome) => {
            // Persist the delivery state on the notification row; terminal
            // failures are audited like sends.
            let terminal = job_outcome.status == "failed";
            let notification_id = row.id;
            let mut am: entity::notification::ActiveModel = row.into();
            if terminal {
                am.status = Set("failed".into());
            }
            am.last_error = Set(job_outcome.error.clone());
            am.updated_at = Set(Utc::now().into());
            if let Err(e) = am.update(db).await {
                tracing::error!("failed to persist notification delivery state: {e}");
            }
            if terminal {
                crate::audit::record(
                    db,
                    None,
                    crate::audit::actions::NOTIFICATION_SEND,
                    Some("notification"),
                    Some(notification_id.to_string()),
                    Some(job.tenant_id),
                    Some(json!({
                        "template": template,
                        "channel": channel,
                        "status": "failed",
                    })),
                )
                .await;
            }
            job_outcome
        }
    }
}

/// Load the notification row for this job, creating a `queued` one if this is
/// the first attempt.
async fn ensure_row(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
    channel: &str,
    template: &str,
    to: &str,
    idem: Option<&str>,
) -> anyhow::Result<entity::notification::Model> {
    if let Some(existing) = Notification::find()
        .filter(entity::notification::Column::BackgroundJobId.eq(job.id))
        .one(db)
        .await?
    {
        return Ok(existing);
    }
    // A previously-failed send with the same natural key is adopted (retried)
    // rather than duplicated — the unique index would reject a second insert.
    if let Some(key) = idem {
        if let Some(prior) = Notification::find()
            .filter(entity::notification::Column::TenantId.eq(job.tenant_id))
            .filter(entity::notification::Column::IdempotencyKey.eq(key))
            .one(db)
            .await?
        {
            let mut am: entity::notification::ActiveModel = prior.clone().into();
            am.background_job_id = Set(Some(job.id));
            am.updated_at = Set(Utc::now().into());
            return Ok(am.update(db).await?);
        }
    }
    let now = Utc::now();
    Ok(entity::notification::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(job.tenant_id),
        channel: Set(channel.into()),
        template_key: Set(template.into()),
        recipient: Set(to.into()),
        status: Set("queued".into()),
        provider_message_id: Set(None),
        subject: Set(None),
        body: Set(None),
        background_job_id: Set(Some(job.id)),
        idempotency_key: Set(idem.map(str::to_string)),
        last_error: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vec<(String, String)> {
        vec![
            ("recipient".into(), "taylor@example.com".into()),
            ("company".into(), "Northwind Property Group".into()),
        ]
    }

    #[test]
    fn renders_platform_default_email() {
        let pairs = vars();
        let map: HashMap<&str, String> =
            pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let r = render(&json!({}), "email", "application_approved", &map).unwrap();
        assert_eq!(
            r.subject.as_deref(),
            Some("Your application with Northwind Property Group has been approved")
        );
        assert!(r.body.contains("taylor@example.com"));
        assert!(r.body.contains("Northwind Property Group"));
    }

    #[test]
    fn renders_sms_variant() {
        let pairs = vars();
        let map: HashMap<&str, String> =
            pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let r = render(&json!({}), "sms", "application_approved", &map).unwrap();
        assert!(r.subject.is_none());
        assert!(r.body.starts_with("Northwind Property Group: good news"));
    }

    #[test]
    fn tenant_override_wins_field_by_field() {
        let pairs = vars();
        let map: HashMap<&str, String> =
            pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let overrides = json!({
            "application_approved": { "subject": "Welcome home, {recipient}!" }
        });
        let r = render(&overrides, "email", "application_approved", &map).unwrap();
        // Overridden subject, default body.
        assert_eq!(
            r.subject.as_deref(),
            Some("Welcome home, taylor@example.com!")
        );
        assert!(r.body.contains("Great news"));

        // A bare-string override replaces the body wholesale.
        let plain = json!({ "application_approved": "Custom body for {recipient}." });
        let r = render(&plain, "email", "application_approved", &map).unwrap();
        assert_eq!(r.body, "Custom body for taylor@example.com.");
    }

    #[test]
    fn unknown_template_is_none_unless_overridden() {
        let map: HashMap<&str, String> = HashMap::new();
        assert!(render(&json!({}), "email", "no_such_template", &map).is_none());
        let overrides = json!({ "no_such_template": "Hello!" });
        assert!(render(&overrides, "email", "no_such_template", &map).is_some());
    }

    #[test]
    fn idempotency_key_needs_owner_context() {
        let legacy = json!({ "template": "application_approved", "to": "a@b.c" });
        assert!(idempotency_key(&legacy, "application_approved", "email").is_none());

        let rich = json!({
            "template": "application_approved",
            "to": "a@b.c",
            "owner_type": "application",
            "owner_id": "6a1c…",
            "trigger": "approved",
        });
        assert_eq!(
            idempotency_key(&rich, "application_approved", "email").as_deref(),
            Some("email:application_approved:application:6a1c…:approved")
        );
    }
}
