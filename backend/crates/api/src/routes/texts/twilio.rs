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

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// What Twilio does with a call to the texting number: say the greeting,
/// then ring `forward` for 20 seconds and report back to `after_url`, or
/// hang up when there's nothing to ring.
pub fn voice_twiml(greeting: &str, forward: Option<&str>, after_url: &str) -> String {
    let say = if greeting.trim().is_empty() {
        String::new()
    } else {
        format!("<Say>{}</Say>", xml_escape(greeting.trim()))
    };
    match forward {
        Some(n) => format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Response>{say}<Dial timeout="20" action="{}" method="POST"><Number>{}</Number></Dial></Response>"#,
            xml_escape(after_url),
            xml_escape(n)
        ),
        None => {
            format!(r#"<?xml version="1.0" encoding="UTF-8"?><Response>{say}<Hangup/></Response>"#)
        }
    }
}

/// Did the rung phone pick up? (`DialCallStatus` from Twilio.)
pub fn answered(dial_status: Option<&str>) -> bool {
    dial_status == Some("completed")
}

async fn greeting_and_forward(state: &AppState, tenant_id: uuid::Uuid) -> (String, Option<String>) {
    use crate::settings as cfg;
    let company = entity::prelude::Tenant::find_by_id(tenant_id)
        .one(&state.db)
        .await
        .ok()
        .flatten()
        .map(|t| t.name)
        .unwrap_or_default();
    let greeting = cfg::get_string(&state.db, tenant_id, cfg::TEXTS_VOICE_GREETING)
        .await
        .replace("{company}", &company);
    let forward = texts::normalize_phone(
        &cfg::get_string(&state.db, tenant_id, cfg::TEXTS_FORWARD_NUMBER).await,
    );
    (greeting, forward)
}

/// `POST /webhooks/twilio/voice?tenant=<slug>` — someone called the texting
/// number. Rings the office phone when one is set; otherwise the call is
/// missed and the caller is texted back.
#[rocket_okapi::openapi(skip)]
#[post("/webhooks/twilio/voice", data = "<body>")]
pub async fn voice(
    state: &State<AppState>,
    tenant: PublicTenant,
    twilio: TwilioRequest,
    body: String,
) -> ApiResult<RawXml<String>> {
    let form = verified_form(state, tenant.tenant_id, &twilio, &body).await?;
    let (greeting, forward) = greeting_and_forward(state, tenant.tenant_id).await;
    let query = twilio
        .origin
        .split_once('?')
        .map(|(_, q)| format!("?{q}"))
        .unwrap_or_default();
    let after = format!(
        "{}/webhooks/twilio/voice/after{query}",
        crate::oauth::public_api_url()
    );
    if forward.is_none() {
        let from = field(&form, "From").unwrap_or_default();
        let sid = field(&form, "CallSid").map(|s| format!("twilio-call:{s}"));
        texts::record_missed_call(&state.db, tenant.tenant_id, from, sid).await?;
    }
    Ok(RawXml(voice_twiml(&greeting, forward.as_deref(), &after)))
}

/// `POST /webhooks/twilio/voice/after?tenant=<slug>` — how the rung phone
/// answered. Anything but a completed call is a missed call.
#[rocket_okapi::openapi(skip)]
#[post("/webhooks/twilio/voice/after", data = "<body>")]
pub async fn voice_after(
    state: &State<AppState>,
    tenant: PublicTenant,
    twilio: TwilioRequest,
    body: String,
) -> ApiResult<RawXml<&'static str>> {
    let form = verified_form(state, tenant.tenant_id, &twilio, &body).await?;
    if !answered(field(&form, "DialCallStatus")) {
        let from = field(&form, "From").unwrap_or_default();
        let sid = field(&form, "CallSid").map(|s| format!("twilio-call:{s}"));
        texts::record_missed_call(&state.db, tenant.tenant_id, from, sid).await?;
    }
    Ok(RawXml(
        r#"<?xml version="1.0" encoding="UTF-8"?><Response><Hangup/></Response>"#,
    ))
}

#[cfg(test)]
mod voice_tests {
    use super::*;

    #[test]
    fn rings_then_reports_back() {
        let x = voice_twiml(
            "Thanks for calling A & B.",
            Some("+15035550100"),
            "https://api.example/webhooks/twilio/voice/after?tenant=nw",
        );
        assert!(x.contains("<Say>Thanks for calling A &amp; B.</Say>"));
        assert!(x.contains(r#"<Dial timeout="20" action="https://api.example/webhooks/twilio/voice/after?tenant=nw" method="POST"><Number>+15035550100</Number></Dial>"#));
    }

    #[test]
    fn hangs_up_with_nothing_to_ring() {
        let x = voice_twiml("", None, "x");
        assert!(x.ends_with("<Response><Hangup/></Response>"));
        assert!(!x.contains("<Say>"));
    }

    #[test]
    fn only_completed_is_answered() {
        assert!(answered(Some("completed")));
        for s in ["no-answer", "busy", "failed", "canceled"] {
            assert!(!answered(Some(s)));
        }
        assert!(!answered(None));
    }
}
