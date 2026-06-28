//! Outbound **email** with a pluggable provider, and a durable record of every
//! send.
//!
//! * `log` (default) — simulates delivery (logs + records), so dev never sends
//!   real mail. Mirrors the platform's "simulated but durable" automation ethos.
//! * `smtp` — real delivery via [`lettre`] over rustls (STARTTLS or implicit TLS).
//!
//! Either way, one `sent_email` row is written (status `sent` / `simulated` /
//! `failed`) so there's an auditable trail, with an optional link to the attached
//! generated document (e.g. the lease PDF).

use crate::config::EmailSettings;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

/// An email to dispatch and record.
pub struct OutboundEmail {
    pub tenant_id: Uuid,
    pub llc_id: Option<Uuid>,
    pub to: String,
    pub cc: Option<String>,
    pub subject: String,
    pub body: String,
    pub template_id: Option<Uuid>,
    pub job_id: Option<Uuid>,
    pub generated_document_id: Option<Uuid>,
}

/// Dispatch `msg` through the configured provider and persist a `sent_email` row.
/// Returns the id of that row. Delivery failures are recorded (status `failed`),
/// not propagated, so a flaky mail server never breaks the calling operation.
pub async fn send(
    user_db: &DatabaseConnection,
    cfg: &EmailSettings,
    msg: OutboundEmail,
) -> Result<Uuid, sea_orm::DbErr> {
    let (provider, status, error) = match cfg.provider.as_str() {
        "smtp" => match deliver_smtp(cfg, &msg).await {
            Ok(()) => ("smtp", "sent", None),
            Err(e) => {
                tracing::error!("smtp send failed: {e:#}");
                ("smtp", "failed", Some(format!("{e:#}")))
            }
        },
        _ => {
            tracing::info!(to = %msg.to, subject = %msg.subject, "email (simulated/log provider)");
            ("log", "simulated", None)
        }
    };

    let now = Utc::now();
    let row = entity::sent_email::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(msg.tenant_id),
        llc_id: Set(msg.llc_id),
        to_address: Set(msg.to),
        cc: Set(msg.cc),
        subject: Set(msg.subject),
        body: Set(msg.body),
        template_id: Set(msg.template_id),
        provider: Set(provider.to_string()),
        status: Set(status.to_string()),
        error: Set(error),
        job_id: Set(msg.job_id),
        generated_document_id: Set(msg.generated_document_id),
        created_at: Set(now.into()),
    };
    let saved = row.insert(user_db).await?;
    Ok(saved.id)
}

async fn deliver_smtp(cfg: &EmailSettings, msg: &OutboundEmail) -> anyhow::Result<()> {
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let host = cfg
        .smtp_host
        .clone()
        .ok_or_else(|| anyhow::anyhow!("SMTP_HOST not configured"))?;

    let mut builder = Message::builder()
        .from(cfg.from.parse()?)
        .to(msg.to.parse()?)
        .subject(msg.subject.clone());
    if let Some(cc) = msg.cc.as_deref().filter(|s| !s.trim().is_empty()) {
        builder = builder.cc(cc.parse()?);
    }
    let email = builder.body(msg.body.clone())?;

    let mut transport = if cfg.smtp_starttls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&host)?
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&host)?
    }
    .port(cfg.smtp_port);

    if let (Some(user), Some(pass)) = (cfg.smtp_user.clone(), cfg.smtp_pass.clone()) {
        transport = transport.credentials(Credentials::new(user, pass));
    }

    transport.build().send(email).await?;
    Ok(())
}
