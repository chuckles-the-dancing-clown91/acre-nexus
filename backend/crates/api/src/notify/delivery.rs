//! **Delivery providers** for email, SMS, and chat — the concrete
//! implementations behind a tenant's configured `notification_provider` rows,
//! all riding the [`crate::providers`] trait.
//!
//! Modern hosted services, one small `match` arm each:
//! * email — **Resend**, **SendGrid**, **Postmark**
//! * sms — **Twilio**
//! * chat — **Slack** / **Discord** incoming webhooks
//!
//! Credentials come from the secrets vault via the provider row's
//! `secret_ref`; non-secret settings (from address, account sid) come from its
//! `config`. Every provider keeps a deterministic `simulate()` (the dev/CI
//! default), and `LIVE_PROVIDERS` remains the platform-level gate that lets
//! `call()` reach the network at all.

use crate::providers::{client, err, Provider, ProviderCtx, ProviderError};
use sea_orm::ConnectionTrait;
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

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

/// Resolve the provider row's vault credential or fail loudly.
async fn credential<C: ConnectionTrait + Sync>(
    ctx: &ProviderCtx<'_, C>,
    row: &entity::notification_provider::Model,
) -> Result<String, ProviderError> {
    let key = row.secret_ref.as_deref().ok_or_else(|| {
        err(format!(
            "{} provider has no credential configured",
            row.kind
        ))
    })?;
    ctx.secret(key)
        .await?
        .ok_or_else(|| err(format!("credential '{key}' missing from the vault")))
}

fn config_str(row: &entity::notification_provider::Model, field: &str) -> Option<String> {
    row.config
        .get(field)
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

async fn expect_success(
    resp: reqwest::Response,
    what: &str,
) -> Result<serde_json::Value, ProviderError> {
    let status = resp.status();
    if !status.is_success() {
        let detail = resp.text().await.unwrap_or_default();
        let detail = detail.chars().take(300).collect::<String>();
        return Err(err(format!("{what} returned HTTP {status}: {detail}")));
    }
    Ok(resp.json().await.unwrap_or(serde_json::Value::Null))
}

// ---------------------------------------------------------------------------
// Simulated fallbacks (no provider configured — the dev/CI default)
// ---------------------------------------------------------------------------

/// Email fallback when the tenant has configured no provider. `call` is
/// intentionally loud so a live deployment can't silently drop mail.
pub struct SimulatedEmail;

#[async_trait::async_trait]
impl Provider for SimulatedEmail {
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
            "no email provider configured — add one under Console → Notifications",
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

/// SMS fallback, same shape as email.
pub struct SimulatedSms;

#[async_trait::async_trait]
impl Provider for SimulatedSms {
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
            "no SMS provider configured — add one under Console → Notifications",
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
// Configured email (Resend / SendGrid / Postmark)
// ---------------------------------------------------------------------------

pub struct EmailDelivery {
    pub row: entity::notification_provider::Model,
}

#[async_trait::async_trait]
impl Provider for EmailDelivery {
    type Request = MessageRequest;
    type Response = MessageResponse;

    fn key(&self) -> &'static str {
        "email"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let api_key = credential(ctx, &self.row).await?;
        let from = config_str(&self.row, "from")
            .ok_or_else(|| err("email provider config is missing 'from'"))?;
        let subject = req.subject.clone().unwrap_or_default();
        let http = client::build_http_client()?;

        let id = match self.row.kind.as_str() {
            "resend" => {
                let resp = http
                    .post("https://api.resend.com/emails")
                    .bearer_auth(&api_key)
                    .json(&json!({
                        "from": from,
                        "to": [req.to],
                        "subject": subject,
                        "text": req.body,
                    }))
                    .send()
                    .await
                    .map_err(|e| err(format!("resend request failed: {e}")))?;
                expect_success(resp, "resend").await?["id"]
                    .as_str()
                    .unwrap_or("accepted")
                    .to_string()
            }
            "sendgrid" => {
                let resp = http
                    .post("https://api.sendgrid.com/v3/mail/send")
                    .bearer_auth(&api_key)
                    .json(&json!({
                        "personalizations": [{ "to": [{ "email": req.to }] }],
                        "from": { "email": from },
                        "subject": subject,
                        "content": [{ "type": "text/plain", "value": req.body }],
                    }))
                    .send()
                    .await
                    .map_err(|e| err(format!("sendgrid request failed: {e}")))?;
                let id = resp
                    .headers()
                    .get("X-Message-Id")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("accepted")
                    .to_string();
                expect_success(resp, "sendgrid").await?;
                id
            }
            "postmark" => {
                let resp = http
                    .post("https://api.postmarkapp.com/email")
                    .header("X-Postmark-Server-Token", &api_key)
                    .json(&json!({
                        "From": from,
                        "To": req.to,
                        "Subject": subject,
                        "TextBody": req.body,
                    }))
                    .send()
                    .await
                    .map_err(|e| err(format!("postmark request failed: {e}")))?;
                expect_success(resp, "postmark").await?["MessageID"]
                    .as_str()
                    .unwrap_or("accepted")
                    .to_string()
            }
            other => return Err(err(format!("unknown email provider kind: {other}"))),
        };
        Ok(MessageResponse {
            provider_message_id: format!("{}:{id}", self.row.kind),
        })
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        tracing::info!(kind = %self.row.kind, to = %req.to, "simulated email sent (provider configured, LIVE_PROVIDERS off)");
        Ok(MessageResponse {
            provider_message_id: format!("sim-{}-{}", self.row.kind, Uuid::new_v4().simple()),
        })
    }
}

// ---------------------------------------------------------------------------
// Configured SMS (Twilio)
// ---------------------------------------------------------------------------

pub struct SmsDelivery {
    pub row: entity::notification_provider::Model,
}

#[async_trait::async_trait]
impl Provider for SmsDelivery {
    type Request = MessageRequest;
    type Response = MessageResponse;

    fn key(&self) -> &'static str {
        "sms"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        if self.row.kind != "twilio" {
            return Err(err(format!("unknown SMS provider kind: {}", self.row.kind)));
        }
        let auth_token = credential(ctx, &self.row).await?;
        let sid = config_str(&self.row, "account_sid")
            .ok_or_else(|| err("twilio config is missing 'account_sid'"))?;
        let from = config_str(&self.row, "from")
            .ok_or_else(|| err("twilio config is missing 'from' (sending number)"))?;

        let http = client::build_http_client()?;
        let resp = http
            .post(format!(
                "https://api.twilio.com/2010-04-01/Accounts/{sid}/Messages.json"
            ))
            .basic_auth(&sid, Some(&auth_token))
            .form(&[
                ("To", req.to.as_str()),
                ("From", from.as_str()),
                ("Body", req.body.as_str()),
            ])
            .send()
            .await
            .map_err(|e| err(format!("twilio request failed: {e}")))?;
        let body = expect_success(resp, "twilio").await?;
        Ok(MessageResponse {
            provider_message_id: format!("twilio:{}", body["sid"].as_str().unwrap_or("accepted")),
        })
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        tracing::info!(kind = %self.row.kind, to = %req.to, "simulated SMS sent (provider configured, LIVE_PROVIDERS off)");
        Ok(MessageResponse {
            provider_message_id: format!("sim-{}-{}", self.row.kind, Uuid::new_v4().simple()),
        })
    }
}

// ---------------------------------------------------------------------------
// Configured chat (Slack / Discord incoming webhooks)
// ---------------------------------------------------------------------------

pub struct ChatDelivery {
    pub row: entity::notification_provider::Model,
}

#[async_trait::async_trait]
impl Provider for ChatDelivery {
    type Request = MessageRequest;
    type Response = MessageResponse;

    fn key(&self) -> &'static str {
        "chat"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        // The webhook URL itself is the credential — Slack/Discord treat it as
        // a secret, so it lives in the vault.
        let webhook_url = credential(ctx, &self.row).await?;
        if !webhook_url.starts_with("https://") {
            return Err(err("chat webhook URL must be https"));
        }
        let text = match &req.subject {
            Some(s) => format!("*{s}*\n{}", req.body),
            None => req.body.clone(),
        };
        let payload = match self.row.kind.as_str() {
            "slack" => json!({ "text": text }),
            "discord" => json!({ "content": text }),
            other => return Err(err(format!("unknown chat provider kind: {other}"))),
        };
        let http = client::build_http_client()?;
        let resp = http
            .post(&webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| err(format!("{} webhook failed: {e}", self.row.kind)))?;
        if !resp.status().is_success() {
            return Err(err(format!(
                "{} webhook returned HTTP {}",
                self.row.kind,
                resp.status()
            )));
        }
        Ok(MessageResponse {
            provider_message_id: format!("{}:delivered", self.row.kind),
        })
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        _ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        tracing::info!(kind = %self.row.kind, "simulated chat message: {}", req.body);
        Ok(MessageResponse {
            provider_message_id: format!("sim-{}-{}", self.row.kind, Uuid::new_v4().simple()),
        })
    }
}
