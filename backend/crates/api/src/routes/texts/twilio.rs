//! Twilio webhooks for two-way texts.
//!
//! * `POST /webhooks/twilio/sms?tenant=<slug>` — "A message comes in" on the
//!   workspace's number: filed in its thread, STOP/START applied, staff told.
//! * `POST /webhooks/twilio/status?tenant=<slug>` — delivery status callbacks:
//!   the matching outbound text becomes `sent` / `failed`.
//!
//! Every request must carry a valid `X-Twilio-Signature` for the workspace's
//! Twilio auth token over the exact public URL (`PUBLIC_API_URL` + path +
//! query) and the posted form — anything else is refused with 403. Twilio
//! handles its own STOP/HELP auto-replies, so the answer is always empty TwiML.

use crate::error::{ApiError, ApiResult};
use crate::providers::ProviderCtx;
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use crate::texts;
use entity::prelude::SmsMessage;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::content::RawXml;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

const EMPTY_TWIML: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Response></Response>"#;

/// The signature header and the path + query Twilio called.
pub struct TwilioRequest {
    signature: Option<String>,
    origin: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for TwilioRequest {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        Outcome::Success(TwilioRequest {
            signature: req
                .headers()
                .get_one("X-Twilio-Signature")
                .map(str::to_string),
            origin: req.uri().to_string(),
        })
    }
}

/// Verify the request against the workspace's Twilio auth token and return
/// the decoded form.
async fn verified_form(
    state: &AppState,
    tenant_id: uuid::Uuid,
    twilio: &TwilioRequest,
    body: &str,
) -> ApiResult<Vec<(String, String)>> {
    let refuse = || ApiError::Forbidden("invalid Twilio signature".into());
    let presented = twilio.signature.as_deref().ok_or_else(refuse)?;
    let provider = crate::notify::default_provider(&state.db, tenant_id, "sms")
        .await
        .filter(|p| p.kind == "twilio")
        .ok_or_else(refuse)?;
    let key = provider.secret_ref.as_deref().ok_or_else(refuse)?;
    let token = ProviderCtx::new(&state.db, tenant_id)
        .secret(key)
        .await
        .ok()
        .flatten()
        .ok_or_else(refuse)?;
    let url = format!("{}{}", crate::oauth::public_api_url(), twilio.origin);
    let form = texts::parse_form(body);
    if !texts::twilio_signature_ok(&token, &url, &form, presented) {
        tracing::warn!(%tenant_id, "rejected a Twilio webhook with a bad signature");
        return Err(refuse());
    }
    Ok(form)
}

fn field<'a>(form: &'a [(String, String)], name: &str) -> Option<&'a str> {
    form.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

/// `POST /webhooks/twilio/sms?tenant=<slug>` — an inbound text.
#[rocket_okapi::openapi(skip)]
#[post("/webhooks/twilio/sms", data = "<body>")]
pub async fn inbound(
    state: &State<AppState>,
    tenant: PublicTenant,
    twilio: TwilioRequest,
    body: String,
) -> ApiResult<RawXml<&'static str>> {
    let form = verified_form(state, tenant.tenant_id, &twilio, &body).await?;
    let from = field(&form, "From").unwrap_or_default();
    let text = field(&form, "Body").unwrap_or_default();
    let sid = field(&form, "MessageSid").map(|s| format!("twilio:{s}"));
    let count = field(&form, "NumMedia")
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(0)
        .min(10);
    let media: Vec<(String, String)> = (0..count)
        .filter_map(|i| {
            let url = field(&form, &format!("MediaUrl{i}"))?;
            let ct = field(&form, &format!("MediaContentType{i}")).unwrap_or("image/jpeg");
            Some((url.to_string(), ct.to_string()))
        })
        .collect();
    texts::record_inbound(&state.db, tenant.tenant_id, from, text, sid, &media).await?;
    Ok(RawXml(EMPTY_TWIML))
}

/// `POST /webhooks/twilio/status?tenant=<slug>` — a delivery status update.
#[rocket_okapi::openapi(skip)]
#[post("/webhooks/twilio/status", data = "<body>")]
pub async fn status(
    state: &State<AppState>,
    tenant: PublicTenant,
    twilio: TwilioRequest,
    body: String,
) -> ApiResult<RawXml<&'static str>> {
    let form = verified_form(state, tenant.tenant_id, &twilio, &body).await?;
    let (Some(sid), Some(status)) = (field(&form, "MessageSid"), field(&form, "MessageStatus"))
    else {
        return Ok(RawXml(EMPTY_TWIML));
    };
    let mapped = match status {
        "delivered" | "sent" => "sent",
        "failed" | "undelivered" => "failed",
        _ => return Ok(RawXml(EMPTY_TWIML)),
    };
    if let Some(m) = SmsMessage::find()
        .filter(entity::sms_message::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::sms_message::Column::ProviderMessageId.eq(format!("twilio:{sid}")))
        .one(&state.db)
        .await?
    {
        let error = field(&form, "ErrorCode").map(|c| format!("Twilio error {c}"));
        let mut am: entity::sms_message::ActiveModel = m.into();
        am.status = Set(mapped.to_string());
        if mapped == "failed" {
            am.error = Set(error);
        }
        am.update(&state.db).await?;
    }
    Ok(RawXml(EMPTY_TWIML))
}
