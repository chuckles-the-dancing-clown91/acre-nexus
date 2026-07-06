//! **Notifications** — templated, multi-channel messaging (roadmap issue #18,
//! expanded with tenant-configurable providers, Web Push, and the in-app
//! inbox).
//!
//! Channels and their job kinds:
//!
//! | channel  | kind         | delivered by |
//! |----------|--------------|--------------|
//! | `email`  | `auto_email` | the tenant's configured provider ([`delivery::EmailDelivery`]: Resend / SendGrid / Postmark) or the simulated fallback |
//! | `sms`    | `auto_sms`   | Twilio ([`delivery::SmsDelivery`]) or the simulated fallback |
//! | `push`   | `auto_push`  | Web Push / VAPID ([`webpush::PushDelivery`]) to every browser subscription the user holds |
//! | `chat`   | `auto_chat`  | Slack / Discord incoming webhook ([`delivery::ChatDelivery`]); skipped when none is configured |
//! | `in_app` | — (written synchronously) | the `notification` table itself; `user_id` + `read_at` power the console inbox |
//!
//! Templates render through the same `{placeholder}` engine as lease documents
//! ([`crate::leasedoc::interpolate`]); platform defaults live here and tenants
//! override per key via `theme.notification_templates`. Every send is
//! persisted to `notification` and audited, and idempotency keys keep retried
//! jobs and duplicate triggers from double-sending. The original
//! `{ "template": …, "to": … }` payload contract is unchanged.

pub mod delivery;
pub mod webpush;

use crate::leasedoc::interpolate;
use crate::modules::JobOutcome;
use crate::providers::{self, ProviderCtx};
use chrono::Utc;
use delivery::{
    ChatDelivery, EmailDelivery, MessageRequest, MessageResponse, SimulatedEmail, SimulatedSms,
    SmsDelivery,
};
use entity::prelude::{Notification, NotificationProvider, Theme};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, Set,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;
use webpush::{PushDelivery, PushRequest};

/// Channels a tenant can configure a delivery provider for, with the provider
/// kinds each accepts. `push` is platform-managed (VAPID) and `in_app` needs
/// no provider, so neither appears here.
pub const PROVIDER_CHANNELS: &[(&str, &[&str])] = &[
    ("email", &["resend", "sendgrid", "postmark"]),
    ("sms", &["twilio"]),
    ("chat", &["slack", "discord"]),
];

/// One default template: rendered bodies per channel, overridable per tenant
/// via `theme.notification_templates`.
pub struct DefaultTemplate {
    pub key: &'static str,
    pub subject: &'static str,
    pub body: &'static str,
    pub sms: &'static str,
}

/// The platform template catalog — every default the engine ships. The
/// templates settings API merges tenant overrides over these and lets a
/// workspace import them as editable DB copies.
pub fn default_templates() -> &'static [DefaultTemplate] {
    DEFAULT_TEMPLATES
}

/// Platform default templates. A tenant override with the same key (a plain
/// body string, or `{ "subject": …, "body": …, "sms": … }`) wins field by
/// field. The `sms` variant doubles as the short body for push, chat, and
/// in-app renditions.
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
    DefaultTemplate {
        key: "application_submitted",
        subject: "New application from {applicant}",
        body: "Hi {recipient},\n\n{applicant} just submitted a rental application. Review it in \
               the applications inbox.\n\n— {company}",
        sms: "New application from {applicant} — review it in the console.",
    },
    DefaultTemplate {
        key: "application_screened",
        subject: "Screening finished for {applicant}: {result}",
        body: "Hi {recipient},\n\nBackground screening for {applicant} has finished with \
               result: {result}. Review the application and make a decision in the \
               applications inbox.\n\n— {company}",
        sms: "Screening finished for {applicant}: {result} — review in the console.",
    },
    DefaultTemplate {
        key: "application_declined",
        subject: "Update on your application with {company}",
        body: "Hi {recipient},\n\nThank you for applying with {company}. After careful \
               review we're unable to move forward with your application at this time.\n\n\
               If you have questions, just reply to this email.\n\n— {company}",
        sms: "{company}: unfortunately we can't move forward with your application at \
              this time.",
    },
    DefaultTemplate {
        key: "adverse_action",
        subject: "Adverse action notice regarding your application",
        body: "Hi {recipient},\n\nThis notice is provided under the Fair Credit Reporting \
               Act (FCRA). Your rental application with {company} was declined based in \
               whole or in part on information in a consumer report furnished by:\n\n\
               {cra_name}\n{cra_contact}\n\nThe agency did not make this decision and \
               cannot explain why it was made. You may obtain a free copy of your report \
               from the agency within 60 days of this notice, and you may dispute any \
               inaccurate or incomplete information with them directly. The full notice \
               is on file with your application.\n\n— {company}",
        sms: "{company}: an adverse-action notice about your application was issued — \
              see your email for your FCRA rights.",
    },
    DefaultTemplate {
        key: "ticket_created",
        subject: "New maintenance ticket: {title}",
        body: "Hi {recipient},\n\nA new {priority}-priority maintenance ticket was opened: \
               {title}. Review it on the maintenance board.\n\n— {company}",
        sms: "New {priority} maintenance ticket: {title}",
    },
    DefaultTemplate {
        key: "test_notification",
        subject: "Test notification from {company}",
        body: "Hi {recipient},\n\nThis is a test notification from {company}. If you're reading \
               this, delivery is working.\n\n— {company}",
        sms: "{company}: test notification — delivery is working.",
    },
    // ---- E-signature envelopes (Phase 2) ----
    DefaultTemplate {
        key: "esign_request",
        subject: "Signature requested: {document_title}",
        body: "Hi {signer},\n\n{company} has requested your signature on \
               \"{document_title}\".\n\nReview and sign here:\n{sign_url}\n\nThis link is \
               unique to you — please do not forward it. By signing you agree to transact \
               electronically (ESIGN/UETA).\n\n— {company}",
        sms: "{company}: your signature is requested on {document_title}. Sign: {sign_url}",
    },
    DefaultTemplate {
        key: "esign_reminder",
        subject: "Reminder — {document_title} is awaiting your signature",
        body: "Hi {signer},\n\nA friendly reminder that \"{document_title}\" from {company} \
               is still awaiting your signature.\n\nReview and sign here:\n{sign_url}\n\n\
               — {company}",
        sms: "{company}: reminder — {document_title} is awaiting your signature. Sign: {sign_url}",
    },
    DefaultTemplate {
        key: "esign_signed_staff",
        subject: "{signer} signed {document_title}",
        body: "Hi {recipient},\n\n{signer} has signed \"{document_title}\" \
               ({signed_count}/{signer_count} signatures in). You'll be notified when \
               everyone has signed.\n\n— {company}",
        sms: "{signer} signed {document_title} ({signed_count}/{signer_count} signatures in).",
    },
    DefaultTemplate {
        key: "esign_completed",
        subject: "Fully signed: {document_title}",
        body: "Hi {signer},\n\nAll parties have now signed \"{document_title}\". The fully \
               executed copy is kept with the lease records at {company} — you can request \
               a copy at any time.\n\n— {company}",
        sms: "{company}: {document_title} is fully signed. The executed copy is on file.",
    },
    DefaultTemplate {
        key: "esign_completed_staff",
        subject: "{document_title} fully signed",
        body: "Hi {recipient},\n\n\"{document_title}\" is fully executed — signed by \
               {signed_by}. The signed PDF is stored on the lease and the lease is now \
               active.\n\n— {company}",
        sms: "{document_title} fully executed — signed by {signed_by}. Lease activated.",
    },
    DefaultTemplate {
        key: "esign_declined_staff",
        subject: "{signer} declined to sign {document_title}",
        body: "Hi {recipient},\n\n{signer} declined to sign \"{document_title}\"{reason_line}. \
               The envelope is closed; you can revise the document and send a new one.\n\n\
               — {company}",
        sms: "{signer} declined to sign {document_title}.",
    },
    DefaultTemplate {
        key: "esign_voided",
        subject: "Signature request cancelled: {document_title}",
        body: "Hi {signer},\n\nThe signature request for \"{document_title}\" from {company} \
               has been cancelled — no further action is needed. Your signing link no longer \
               works.\n\n— {company}",
        sms: "{company}: the signature request for {document_title} was cancelled.",
    },
    DefaultTemplate {
        key: "payment_receipt",
        subject: "Payment received — {amount}",
        body: "Hi {recipient},\n\nWe received your payment of {amount}. Your receipt number \
               is {receipt_number}; a PDF copy is kept with your lease records.\n\nThank you!\n\n\
               — {company}",
        sms: "{company}: payment of {amount} received. Receipt {receipt_number}.",
    },
    DefaultTemplate {
        key: "payment_failed",
        subject: "Your payment could not be processed",
        body: "Hi {recipient},\n\nYour payment of {amount} could not be processed: {reason}. \
               No money was taken. Please try again with another payment method, or contact \
               us if the problem persists.\n\n— {company}",
        sms: "{company}: your payment of {amount} failed ({reason}). Please try another method.",
    },
    DefaultTemplate {
        key: "payment_received",
        subject: "Payment received: {amount} from {resident}",
        body: "Hi {recipient},\n\n{resident} paid {amount}. The payment has settled, posted \
               to the ledger, and a receipt was issued.\n\n— {company}",
        sms: "Payment settled: {amount} from {resident}.",
    },
    DefaultTemplate {
        key: "late_fee_applied",
        subject: "A late fee was applied to your account",
        body: "Hi {recipient},\n\nRent for {month} is past its grace period, and a late fee \
               of {amount} has been applied to your account per your lease terms. Paying \
               your outstanding balance stops further fees.\n\n— {company}",
        sms: "{company}: a {amount} late fee was applied for {month}. Please pay your balance.",
    },
    DefaultTemplate {
        key: "payout_paid",
        subject: "Owner payout sent: {amount}",
        body: "Hi {recipient},\n\nAn owner payout of {amount} was executed and the statement \
               is filed on the entity. The ledger entry is linked from the payout record.\n\n\
               — {company}",
        sms: "Owner payout of {amount} sent.",
    },
];

/// A rendered, ready-to-send message. `subject` doubles as the title for
/// push/in-app renditions.
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
    // A bare-string override is a body for every channel at once.
    let over_plain = over.and_then(|o| o.as_str()).map(str::to_string);

    // sms/chat/push/in_app use the short (`sms`) variant; email the long body.
    let body_template = match channel {
        "email" => str_field("body")
            .or_else(|| over_plain.clone())
            .or(default.map(|d| d.body.to_string()))?,
        _ => str_field("sms")
            .or_else(|| over_plain.clone())
            .or(default.map(|d| d.sms.to_string()))?,
    };
    // sms and chat are bare text; email, push, and in_app carry a subject/title.
    let subject = match channel {
        "sms" | "chat" => None,
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

/// Tenant branding + template overrides for rendering.
async fn tenant_context(db: &impl ConnectionTrait, tenant_id: Uuid) -> (String, serde_json::Value) {
    let theme = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
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
    (company, overrides)
}

/// Interpolation vars: built-ins + any string vars the caller provided.
fn build_vars(
    recipient: &str,
    company: &str,
    template: &str,
    extra: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = vec![
        ("recipient".into(), recipient.to_string()),
        ("company".into(), company.to_string()),
        ("template".into(), template.to_string()),
    ];
    if let Some(extra) = extra {
        for (k, v) in extra {
            let val = v
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| v.to_string());
            pairs.push((k.clone(), val));
        }
    }
    pairs
}

/// The tenant's delivery provider for a channel: the default first, else the
/// oldest enabled one, else `None` (→ simulated fallback).
pub async fn default_provider(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    channel: &str,
) -> Option<entity::notification_provider::Model> {
    NotificationProvider::find()
        .filter(entity::notification_provider::Column::TenantId.eq(tenant_id))
        .filter(entity::notification_provider::Column::Channel.eq(channel))
        .filter(entity::notification_provider::Column::Enabled.eq(true))
        .order_by_desc(entity::notification_provider::Column::IsDefault)
        .order_by_asc(entity::notification_provider::Column::CreatedAt)
        .one(db)
        .await
        .ok()
        .flatten()
}

/// The provider a job routes through: an explicit `provider_id` in the payload
/// (the per-provider test button) wins over the channel default.
async fn provider_for_job(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    channel: &str,
    payload: &serde_json::Value,
) -> Option<entity::notification_provider::Model> {
    if let Some(pid) = payload
        .get("provider_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        if let Ok(Some(p)) = NotificationProvider::find_by_id(pid)
            .filter(entity::notification_provider::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
        {
            if p.channel == channel {
                return Some(p);
            }
        }
    }
    default_provider(db, tenant_id, channel).await
}

// ---------------------------------------------------------------------------
// Job handling
// ---------------------------------------------------------------------------

/// The natural de-duplication key for a send, when the payload carries enough
/// context to build one (`owner_type` + `owner_id`, plus an optional
/// `trigger`). Legacy payloads without owner context fall back to per-job
/// dedup via `notification.background_job_id`. User-directed channels get a
/// per-user key so a broadcast to N users isn't self-deduping.
fn idempotency_key(payload: &serde_json::Value, template: &str, channel: &str) -> Option<String> {
    let owner_type = payload.get("owner_type")?.as_str()?;
    let owner_id = payload.get("owner_id")?.as_str()?;
    let trigger = payload
        .get("trigger")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let user_suffix = payload
        .get("user_id")
        .and_then(|v| v.as_str())
        .map(|u| format!(":{u}"))
        .unwrap_or_default();
    Some(format!(
        "{channel}:{template}:{owner_type}:{owner_id}:{trigger}{user_suffix}"
    ))
}

/// Advance one `auto_email` / `auto_sms` / `auto_push` / `auto_chat` job:
/// render, deliver via the tenant's configured provider (or the simulated
/// fallback), persist the `notification` row, audit, and map provider errors
/// onto the retry budget. Called from the integrations module's `handle_job`.
pub async fn handle_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let channel = match job.kind.as_str() {
        "auto_sms" => "sms",
        "auto_push" => "push",
        "auto_chat" => "chat",
        _ => "email",
    };
    let Some(template) = job.payload.get("template").and_then(|v| v.as_str()) else {
        return JobOutcome::failed("notification payload missing 'template'");
    };

    // Per-channel addressing.
    let user_id = job
        .payload
        .get("user_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    let provider_row = provider_for_job(db, job.tenant_id, channel, &job.payload).await;
    let to: String = match channel {
        "push" => {
            if user_id.is_none() {
                return JobOutcome::failed("push payload missing 'user_id'");
            }
            job.payload
                .get("to")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| user_id.unwrap().to_string())
        }
        "chat" => match &provider_row {
            Some(p) => p.kind.clone(),
            // Chat is opt-in: no provider means nothing to deliver.
            None => {
                return JobOutcome::completed(json!({
                    "skipped": true,
                    "reason": "no chat provider configured",
                }))
            }
        },
        _ => match job.payload.get("to").and_then(|v| v.as_str()) {
            Some(t) => t.to_string(),
            None => return JobOutcome::failed("notification payload missing 'to'"),
        },
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

    let (company, overrides) = tenant_context(db, job.tenant_id).await;
    let pairs = build_vars(
        &to,
        &company,
        template,
        job.payload.get("vars").and_then(|v| v.as_object()),
    );
    let vars: HashMap<&str, String> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();

    let Some(rendered) = render(&overrides, channel, template, &vars) else {
        return JobOutcome::failed(format!(
            "unknown notification template '{template}' (no platform default, no tenant override)"
        ));
    };

    // Ensure the notification row exists (find by job first so a retry updates
    // rather than duplicates).
    let row = match ensure_row(db, job, channel, template, &to, idem.as_deref(), user_id).await {
        Ok(r) => r,
        Err(e) => return JobOutcome::retry(providers::backoff(job.attempts), e.to_string()),
    };

    // Deliver through the provider framework (#16), routed to the tenant's
    // configured provider when one exists.
    let ctx = ProviderCtx::new(db, job.tenant_id);
    let req = MessageRequest {
        to: to.clone(),
        subject: rendered.subject.clone(),
        body: rendered.body.clone(),
    };
    let outcome: Result<MessageResponse, JobOutcome> = match channel {
        "sms" => match provider_row {
            Some(p) => providers::run(&SmsDelivery { row: p }, &ctx, job, &req).await,
            None => providers::run(&SimulatedSms, &ctx, job, &req).await,
        },
        "chat" => {
            let p = provider_row.expect("chat handled above when unconfigured");
            providers::run(&ChatDelivery { row: p }, &ctx, job, &req).await
        }
        "push" => {
            let push_req = PushRequest {
                user_id: user_id.expect("push validated above"),
                title: rendered.subject.clone().unwrap_or_else(|| company.clone()),
                body: rendered.body.clone(),
            };
            providers::run(&PushDelivery, &ctx, job, &push_req).await
        }
        _ => match provider_row {
            Some(p) => providers::run(&EmailDelivery { row: p }, &ctx, job, &req).await,
            None => providers::run(&SimulatedEmail, &ctx, job, &req).await,
        },
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
#[allow(clippy::too_many_arguments)]
async fn ensure_row(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
    channel: &str,
    template: &str,
    to: &str,
    idem: Option<&str>,
    user_id: Option<Uuid>,
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
        user_id: Set(user_id),
        read_at: Set(None),
        last_error: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?)
}

// ---------------------------------------------------------------------------
// In-app inbox + broadcast fan-out
// ---------------------------------------------------------------------------

/// Write one in-app notification directly (no provider, no job — the row *is*
/// the delivery). Silently skips duplicates on the idempotency key.
#[allow(clippy::too_many_arguments)]
pub async fn in_app(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user: &entity::user::Model,
    template: &str,
    vars_json: &serde_json::Value,
    owner: Option<(&str, Uuid)>,
    trigger: &str,
) -> Option<Uuid> {
    let (company, overrides) = tenant_context(db, tenant_id).await;
    let pairs = build_vars(&user.email, &company, template, vars_json.as_object());
    let vars: HashMap<&str, String> = pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
    let rendered = render(&overrides, "in_app", template, &vars)?;

    let idem =
        owner.map(|(otype, oid)| format!("in_app:{template}:{otype}:{oid}:{trigger}:{}", user.id));
    let now = Utc::now();
    let row = entity::notification::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        channel: Set("in_app".into()),
        template_key: Set(template.into()),
        recipient: Set(user.email.clone()),
        status: Set("sent".into()),
        provider_message_id: Set(None),
        subject: Set(rendered.subject),
        body: Set(Some(rendered.body)),
        background_job_id: Set(None),
        idempotency_key: Set(idem),
        user_id: Set(Some(user.id)),
        read_at: Set(None),
        last_error: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    match row.insert(db).await {
        Ok(saved) => Some(saved.id),
        // A duplicate natural key means this trigger already notified the user.
        Err(e) => {
            tracing::debug!("in-app notification skipped (likely duplicate): {e}");
            None
        }
    }
}

/// Fan a tenant event out to every staff member holding `permission_key`:
/// an in-app inbox entry each (written now), a Web Push job each, and one
/// chat message when a chat provider is configured. This is the "integrated
/// notifications" path real events call. `exclude_user` skips the actor who
/// caused the event (no point notifying yourself).
#[allow(clippy::too_many_arguments)]
pub async fn notify_staff(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    permission_key: &str,
    template: &str,
    vars_json: serde_json::Value,
    owner: Option<(&str, Uuid)>,
    trigger: &str,
    exclude_user: Option<Uuid>,
) {
    let users = match staff_with_permission(db, tenant_id, permission_key).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("notify_staff recipient lookup failed: {e}");
            return;
        }
    };
    let users: Vec<entity::user::Model> = users
        .into_iter()
        .filter(|u| Some(u.id) != exclude_user)
        .collect();

    let owner_fields = |payload: &mut serde_json::Map<String, serde_json::Value>| {
        if let Some((otype, oid)) = owner {
            payload.insert("owner_type".into(), json!(otype));
            payload.insert("owner_id".into(), json!(oid.to_string()));
        }
        payload.insert("trigger".into(), json!(trigger));
    };

    for user in &users {
        // Inbox entry, immediately visible.
        in_app(db, tenant_id, user, template, &vars_json, owner, trigger).await;

        // Web push riding the durable queue, one job per user so retries are
        // isolated per recipient.
        let mut payload = serde_json::Map::new();
        payload.insert("template".into(), json!(template));
        payload.insert("to".into(), json!(user.email));
        payload.insert("user_id".into(), json!(user.id.to_string()));
        payload.insert("vars".into(), vars_json.clone());
        owner_fields(&mut payload);
        if let Err(e) = crate::scheduler::enqueue(
            db,
            tenant_id,
            "auto_push",
            serde_json::Value::Object(payload),
            0,
        )
        .await
        {
            tracing::error!("failed to enqueue auto_push: {e}");
        }
    }

    // One chat message per event (not per user); the handler no-ops if the
    // tenant never configured a chat provider.
    if default_provider(db, tenant_id, "chat").await.is_some() {
        let mut payload = serde_json::Map::new();
        payload.insert("template".into(), json!(template));
        payload.insert("vars".into(), vars_json.clone());
        owner_fields(&mut payload);
        if let Err(e) = crate::scheduler::enqueue(
            db,
            tenant_id,
            "auto_chat",
            serde_json::Value::Object(payload),
            0,
        )
        .await
        {
            tracing::error!("failed to enqueue auto_chat: {e}");
        }
    }

    crate::audit::record(
        db,
        None,
        crate::audit::actions::NOTIFICATION_BROADCAST,
        owner.map(|(t, _)| t),
        owner.map(|(_, id)| id.to_string()),
        Some(tenant_id),
        Some(json!({
            "template": template,
            "trigger": trigger,
            "recipients": users.len(),
        })),
    )
    .await;
}

/// Active tenant users holding `permission_key` through any of their roles.
async fn staff_with_permission(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    permission_key: &str,
) -> anyhow::Result<Vec<entity::user::Model>> {
    let role_ids: Vec<Uuid> = entity::prelude::RolePermission::find()
        .filter(entity::role_permission::Column::Permission.eq(permission_key))
        .all(db)
        .await?
        .into_iter()
        .map(|rp| rp.role_id)
        .collect();
    if role_ids.is_empty() {
        return Ok(vec![]);
    }
    let mut user_ids: Vec<Uuid> = entity::prelude::UserRole::find()
        .filter(entity::user_role::Column::TenantId.eq(tenant_id))
        .filter(entity::user_role::Column::RoleId.is_in(role_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|ur| ur.user_id)
        .collect();
    user_ids.sort();
    user_ids.dedup();
    if user_ids.is_empty() {
        return Ok(vec![]);
    }
    Ok(entity::prelude::User::find()
        .filter(entity::user::Column::Id.is_in(user_ids))
        .filter(entity::user::Column::Status.eq("active"))
        .all(db)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> Vec<(String, String)> {
        vec![
            ("recipient".into(), "taylor@example.com".into()),
            ("company".into(), "Northwind Property Group".into()),
            ("applicant".into(), "Casey Jones".into()),
        ]
    }

    fn map(pairs: &[(String, String)]) -> HashMap<&str, String> {
        pairs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect()
    }

    #[test]
    fn renders_platform_default_email() {
        let pairs = vars();
        let r = render(&json!({}), "email", "application_approved", &map(&pairs)).unwrap();
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
        let r = render(&json!({}), "sms", "application_approved", &map(&pairs)).unwrap();
        assert!(r.subject.is_none());
        assert!(r.body.starts_with("Northwind Property Group: good news"));
    }

    #[test]
    fn push_and_in_app_get_title_plus_short_body() {
        let pairs = vars();
        for channel in ["push", "in_app"] {
            let r = render(&json!({}), channel, "application_submitted", &map(&pairs)).unwrap();
            assert_eq!(
                r.subject.as_deref(),
                Some("New application from Casey Jones")
            );
            assert_eq!(
                r.body,
                "New application from Casey Jones — review it in the console."
            );
        }
    }

    #[test]
    fn chat_uses_short_body_without_subject() {
        let pairs = vars();
        let r = render(&json!({}), "chat", "test_notification", &map(&pairs)).unwrap();
        assert!(r.subject.is_none());
        assert!(r.body.contains("test notification"));
    }

    #[test]
    fn esign_templates_render_the_signing_link() {
        let pairs = vec![
            ("recipient".into(), "jordan@example.com".into()),
            ("company".into(), "Northwind Property Group".into()),
            ("signer".into(), "Jordan Renter".into()),
            (
                "document_title".into(),
                "Residential Lease Agreement".into(),
            ),
            (
                "sign_url".into(),
                "https://app.example.com/sign/tok123?tenant=northwind".into(),
            ),
        ];
        let r = render(&json!({}), "email", "esign_request", &map(&pairs)).unwrap();
        assert_eq!(
            r.subject.as_deref(),
            Some("Signature requested: Residential Lease Agreement")
        );
        assert!(r.body.contains("Hi Jordan Renter"));
        assert!(r
            .body
            .contains("https://app.example.com/sign/tok123?tenant=northwind"));

        let sms = render(&json!({}), "sms", "esign_request", &map(&pairs)).unwrap();
        assert!(sms.subject.is_none());
        assert!(sms
            .body
            .contains("Sign: https://app.example.com/sign/tok123?tenant=northwind"));

        // The reminder + completion variants also resolve.
        for key in [
            "esign_reminder",
            "esign_signed_staff",
            "esign_completed",
            "esign_completed_staff",
            "esign_declined_staff",
            "esign_voided",
        ] {
            assert!(
                render(&json!({}), "email", key, &map(&pairs)).is_some(),
                "template {key} missing"
            );
        }
    }

    #[test]
    fn tenant_override_wins_field_by_field() {
        let pairs = vars();
        let overrides = json!({
            "application_approved": { "subject": "Welcome home, {recipient}!" }
        });
        let r = render(&overrides, "email", "application_approved", &map(&pairs)).unwrap();
        // Overridden subject, default body.
        assert_eq!(
            r.subject.as_deref(),
            Some("Welcome home, taylor@example.com!")
        );
        assert!(r.body.contains("Great news"));

        // A bare-string override replaces the body wholesale.
        let plain = json!({ "application_approved": "Custom body for {recipient}." });
        let r = render(&plain, "email", "application_approved", &map(&pairs)).unwrap();
        assert_eq!(r.body, "Custom body for taylor@example.com.");
    }

    #[test]
    fn unknown_template_is_none_unless_overridden() {
        let empty: HashMap<&str, String> = HashMap::new();
        assert!(render(&json!({}), "email", "no_such_template", &empty).is_none());
        let overrides = json!({ "no_such_template": "Hello!" });
        assert!(render(&overrides, "email", "no_such_template", &empty).is_some());
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

    #[test]
    fn idempotency_key_is_per_user_for_directed_channels() {
        let a = json!({
            "template": "application_submitted",
            "owner_type": "application", "owner_id": "X", "trigger": "submitted",
            "user_id": "user-a",
        });
        let b = json!({
            "template": "application_submitted",
            "owner_type": "application", "owner_id": "X", "trigger": "submitted",
            "user_id": "user-b",
        });
        let ka = idempotency_key(&a, "application_submitted", "push").unwrap();
        let kb = idempotency_key(&b, "application_submitted", "push").unwrap();
        assert_ne!(ka, kb, "a broadcast must not dedupe across recipients");
    }

    #[test]
    fn provider_channel_catalog_is_consistent() {
        for (channel, kinds) in PROVIDER_CHANNELS {
            assert!(!kinds.is_empty(), "channel {channel} has no kinds");
        }
        assert!(PROVIDER_CHANNELS.iter().any(|(c, _)| *c == "email"));
        assert!(PROVIDER_CHANNELS.iter().all(|(c, _)| *c != "push"));
    }
}
