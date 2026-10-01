//! **Two-way texts** (Vantedge phase 2) — the shared logic behind the console
//! text inbox, the Twilio webhooks and the STOP guard on every outbound text.
//!
//! * One [`entity::sms_thread`] per phone number per workspace, matched to the
//!   lease whose resident has that number when there is one.
//! * Inbound texts land in the thread (unread, reopened) and notify staff with
//!   `message:read`. Every outbound text — console replies *and* notification
//!   templates — is written to the same thread, so the office sees the whole
//!   conversation, reminders included.
//! * **STOP always wins.** STOP / STOPALL / UNSUBSCRIBE / CANCEL / END / QUIT and
//!   the Spanish ALTO / PARAR / BAJA set `opted_out_at`; START / UNSTOP (and YES,
//!   only from a stopped number) clear it. While it's set, no text goes out to
//!   that number — the notification job completes as `skipped: opted_out`.

use base64::Engine;
use chrono::Utc;
use entity::prelude::{Lease, SmsMessage, SmsThread};
use hmac::{Hmac, Mac};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};
use sha1::Sha1;
use uuid::Uuid;

/// Longest text body accepted from the console (Twilio splits at 1600).
pub const MAX_TEXT_CHARS: usize = 1_600;

/// What a text means beyond its words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Stop,
    Start,
    Help,
    None,
}

/// Classify an inbound body. Only a message that *is* the keyword counts
/// ("stop" yes, "please don't stop by" no). `YES` means START only when the
/// number had stopped — otherwise it's just someone saying yes.
pub fn classify(body: &str, currently_opted_out: bool) -> Keyword {
    let word: String = body
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .to_uppercase();
    match word.as_str() {
        "STOP" | "STOPALL" | "STOP ALL" | "UNSUBSCRIBE" | "CANCEL" | "END" | "QUIT" | "OPTOUT"
        | "OPT OUT" | "REVOKE" | "ALTO" | "PARAR" | "BAJA" => Keyword::Stop,
        "START" | "UNSTOP" | "SUBSCRIBE" | "OPTIN" | "OPT IN" => Keyword::Start,
        "YES" | "SI" | "SÍ" if currently_opted_out => Keyword::Start,
        "HELP" | "INFO" | "AYUDA" => Keyword::Help,
        _ => Keyword::None,
    }
}

/// Normalise a phone number to E.164. US/Canada 10-digit and 1+10-digit numbers
/// get `+1`; anything already starting with `+` keeps its country code.
pub fn normalize_phone(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if raw.starts_with('+') {
        return (8..=15)
            .contains(&digits.len())
            .then(|| format!("+{digits}"));
    }
    match digits.len() {
        10 => Some(format!("+1{digits}")),
        11 if digits.starts_with('1') => Some(format!("+{digits}")),
        _ => None,
    }
}

/// Decode an `application/x-www-form-urlencoded` body (what Twilio posts).
pub fn parse_form(body: &str) -> Vec<(String, String)> {
    fn hex(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    fn decode(s: &str) -> String {
        let bytes = s.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'+' => out.push(b' '),
                b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    (Some(h), Some(l)) => {
                        out.push(h * 16 + l);
                        i += 2;
                    }
                    _ => out.push(b'%'),
                },
                b => out.push(b),
            }
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }
    body.split('&')
        .filter(|p| !p.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((k, v)) => (decode(k), decode(v)),
            None => (decode(pair), String::new()),
        })
        .collect()
}

/// Twilio's request signature: base64(HMAC-SHA1(auth token, URL + every POST
/// param's name and value, sorted by name)).
pub fn twilio_signature(auth_token: &str, url: &str, params: &[(String, String)]) -> String {
    let mut sorted: Vec<&(String, String)> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut data = url.to_string();
    for (k, v) in sorted {
        data.push_str(k);
        data.push_str(v);
    }
    let mut mac =
        Hmac::<Sha1>::new_from_slice(auth_token.as_bytes()).expect("HMAC accepts any key length");
    mac.update(data.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

/// Constant-time check of a presented `X-Twilio-Signature`.
pub fn twilio_signature_ok(
    auth_token: &str,
    url: &str,
    params: &[(String, String)],
    presented: &str,
) -> bool {
    let expected = twilio_signature(auth_token, url, params);
    let (a, b) = (expected.as_bytes(), presented.trim().as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// A short single-line preview for the inbox list.
pub fn preview(body: &str) -> String {
    let flat: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 120 {
        format!("{}…", flat.chars().take(119).collect::<String>())
    } else {
        flat
    }
}

/// The workspace's thread for `phone` (E.164), if there is one.
pub async fn find_thread(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    phone: &str,
) -> Result<Option<entity::sms_thread::Model>, DbErr> {
    SmsThread::find()
        .filter(entity::sms_thread::Column::TenantId.eq(tenant_id))
        .filter(entity::sms_thread::Column::Phone.eq(phone))
        .one(db)
        .await
}

/// Whether `phone` has texted STOP to this workspace (and not START since).
pub async fn is_opted_out(db: &impl ConnectionTrait, tenant_id: Uuid, phone: &str) -> bool {
    let Some(phone) = normalize_phone(phone) else {
        return false;
    };
    matches!(
        find_thread(db, tenant_id, &phone).await,
        Ok(Some(t)) if t.opted_out_at.is_some()
    )
}

/// The lease whose resident has this number, preferring an active lease.
async fn match_lease(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    phone: &str,
) -> Result<Option<entity::lease::Model>, DbErr> {
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .filter(entity::lease::Column::TenantPhone.is_not_null())
        .all(db)
        .await?;
    let mut matches: Vec<entity::lease::Model> = leases
        .into_iter()
        .filter(|l| {
            l.tenant_phone
                .as_deref()
                .and_then(normalize_phone)
                .as_deref()
                == Some(phone)
        })
        .collect();
    matches.sort_by_key(|l| {
        (
            l.status != "active",
            std::cmp::Reverse(l.start_date.clone()),
        )
    });
    Ok(matches.into_iter().next())
}

/// Find or open the thread for `phone` (E.164), matching it to a resident.
pub async fn ensure_thread(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    phone: &str,
) -> Result<entity::sms_thread::Model, DbErr> {
    if let Some(t) = find_thread(db, tenant_id, phone).await? {
        return Ok(t);
    }
    let lease = match_lease(db, tenant_id, phone).await?;
    let now = Utc::now();
    entity::sms_thread::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        phone: Set(phone.to_string()),
        lease_id: Set(lease.as_ref().map(|l| l.id)),
        user_id: Set(None),
        display_name: Set(lease.map(|l| l.tenant_name)),
        status: Set("open".into()),
        unread_count: Set(0),
        last_preview: Set(None),
        last_message_at: Set(None),
        opted_out_at: Set(None),
        assigned_user_id: Set(None),
        marketing_opt_in_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
}

/// Record a text from `from` (any format): file it in the thread, apply
/// STOP/START, mark the thread unread and open, and let staff know. Photos
/// (`media`: url and content type) are fetched and filed by a job. Then the
/// automatic answers in [`crate::text_auto`] run. Returns the updated thread
/// (`None` for a number that can't be read).
pub async fn record_inbound(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    from: &str,
    body: &str,
    provider_message_id: Option<String>,
    media: &[(String, String)],
) -> Result<Option<entity::sms_thread::Model>, DbErr> {
    let Some(phone) = normalize_phone(from) else {
        return Ok(None);
    };
    let thread = ensure_thread(db, tenant_id, &phone).await?;
    let keyword = classify(body, thread.opted_out_at.is_some());
    let now = Utc::now();

    let message = entity::sms_message::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        thread_id: Set(thread.id),
        direction: Set("in".into()),
        body: Set(body.to_string()),
        status: Set("received".into()),
        provider_message_id: Set(provider_message_id),
        template_key: Set(None),
        sent_by_user_id: Set(None),
        media_count: Set(media.len() as i32),
        media: Set(serde_json::json!([])),
        error: Set(None),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;

    let mut am: entity::sms_thread::ActiveModel = thread.clone().into();
    am.unread_count = Set(thread.unread_count + 1);
    am.status = Set("open".into());
    am.last_preview = Set(Some(preview(body)));
    am.last_message_at = Set(Some(now.into()));
    am.updated_at = Set(now.into());
    match keyword {
        Keyword::Stop => am.opted_out_at = Set(Some(now.into())),
        Keyword::Start => am.opted_out_at = Set(None),
        _ => {}
    }
    let thread = am.update(db).await?;

    let action = match keyword {
        Keyword::Stop => crate::audit::actions::SMS_OPT_OUT,
        Keyword::Start => crate::audit::actions::SMS_OPT_IN,
        _ => crate::audit::actions::SMS_RECEIVE,
    };
    crate::audit::record(
        db,
        None,
        action,
        Some("sms_thread"),
        Some(thread.id.to_string()),
        Some(tenant_id),
        Some(serde_json::json!({ "message_id": message.id })),
    )
    .await;

    // Tell the office — inbox entry + push for everyone who reads messages.
    let who = thread
        .display_name
        .clone()
        .unwrap_or_else(|| thread.phone.clone());
    crate::notify::notify_staff(
        db,
        tenant_id,
        "message:read",
        "text_received",
        serde_json::json!({ "sender": who, "preview": preview(body) }),
        Some(("sms_message", message.id)),
        "received",
        None,
    )
    .await;

    if !media.is_empty() {
        let items: Vec<serde_json::Value> = media
            .iter()
            .map(|(url, ct)| serde_json::json!({ "url": url, "content_type": ct }))
            .collect();
        if let Err(e) = crate::scheduler::enqueue(
            db,
            tenant_id,
            "sms_media",
            serde_json::json!({ "message_id": message.id, "media": items }),
            0,
        )
        .await
        {
            tracing::error!("failed to queue text photos: {e}");
        }
    }
    if keyword == Keyword::None {
        crate::text_auto::after_inbound(db, tenant_id, &thread, body).await;
    }

    Ok(Some(thread))
}

/// A text going out, to be filed in its thread.
pub struct Outbound<'a> {
    pub to: &'a str,
    pub body: &'a str,
    pub status: &'a str,
    pub provider_message_id: Option<String>,
    pub template_key: Option<String>,
    pub sent_by_user_id: Option<Uuid>,
    pub error: Option<String>,
}

/// File an outbound text in the thread for its number (opening one if needed).
/// Doesn't touch the unread count. `Ok(None)` for an unusable number.
pub async fn record_outbound(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    out: Outbound<'_>,
) -> Result<Option<entity::sms_message::Model>, DbErr> {
    let Some(phone) = normalize_phone(out.to) else {
        return Ok(None);
    };
    let thread = ensure_thread(db, tenant_id, &phone).await?;
    let now = Utc::now();
    let message = entity::sms_message::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        thread_id: Set(thread.id),
        direction: Set("out".into()),
        body: Set(out.body.to_string()),
        status: Set(out.status.to_string()),
        provider_message_id: Set(out.provider_message_id),
        template_key: Set(out.template_key),
        sent_by_user_id: Set(out.sent_by_user_id),
        media_count: Set(0),
        media: Set(serde_json::json!([])),
        error: Set(out.error),
        created_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    let mut am: entity::sms_thread::ActiveModel = thread.into();
    am.last_preview = Set(Some(preview(out.body)));
    am.last_message_at = Set(Some(now.into()));
    am.updated_at = Set(now.into());
    am.update(db).await?;
    Ok(Some(message))
}

/// Update a console-typed text once its delivery job finishes.
pub async fn set_message_status(
    db: &impl ConnectionTrait,
    message_id: Uuid,
    status: &str,
    provider_message_id: Option<String>,
    error: Option<String>,
) -> Result<(), DbErr> {
    if let Some(m) = SmsMessage::find_by_id(message_id).one(db).await? {
        let mut am: entity::sms_message::ActiveModel = m.into();
        am.status = Set(status.to_string());
        if provider_message_id.is_some() {
            am.provider_message_id = Set(provider_message_id);
        }
        am.error = Set(error);
        am.update(db).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_words_including_spanish() {
        for w in [
            "STOP",
            "stop",
            " Stop. ",
            "unsubscribe",
            "ALTO",
            "parar",
            "Baja!",
            "QUIT",
        ] {
            assert_eq!(classify(w, false), Keyword::Stop, "{w}");
        }
        assert_eq!(classify("please don't stop by today", false), Keyword::None);
    }

    #[test]
    fn start_and_the_yes_rule() {
        assert_eq!(classify("START", true), Keyword::Start);
        assert_eq!(classify("unstop", false), Keyword::Start);
        assert_eq!(classify("yes", true), Keyword::Start);
        assert_eq!(
            classify("yes", false),
            Keyword::None,
            "YES is only START from a stopped number"
        );
        assert_eq!(classify("ayuda", false), Keyword::Help);
    }

    #[test]
    fn phone_normalisation() {
        assert_eq!(
            normalize_phone("(760) 555-1234").as_deref(),
            Some("+17605551234")
        );
        assert_eq!(
            normalize_phone("1-760-555-1234").as_deref(),
            Some("+17605551234")
        );
        assert_eq!(
            normalize_phone("+44 20 7946 0958").as_deref(),
            Some("+442079460958")
        );
        assert_eq!(normalize_phone("555-1234"), None);
        assert_eq!(normalize_phone(""), None);
    }

    #[test]
    fn form_decoding() {
        let p = parse_form("From=%2B17605551234&Body=Sink+is+leaking%21&NumMedia=0&Empty=");
        assert_eq!(p[0], ("From".into(), "+17605551234".into()));
        assert_eq!(p[1], ("Body".into(), "Sink is leaking!".into()));
        assert_eq!(p[3], ("Empty".into(), String::new()));
        // A stray % is kept, not panicked on.
        assert_eq!(parse_form("Body=100%")[0].1, "100%");
        assert_eq!(parse_form("Body=%E2%9C%93")[0].1, "✓");
    }

    #[test]
    fn twilio_signature_matches_the_documented_example() {
        // The worked example from Twilio's "Webhooks security" docs.
        let url = "https://example.com/myapp.php?foo=1&bar=2";
        let params: Vec<(String, String)> = [
            ("CallSid", "CA1234567890ABCDE"),
            ("Caller", "+14158675310"),
            ("Digits", "1234"),
            ("From", "+14158675310"),
            ("To", "+18005551212"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let sig = twilio_signature("12345", url, &params);
        assert_eq!(sig, "L/OH5YylLD5NRKLltdqwSvS0BnU=");
        assert!(twilio_signature_ok("12345", url, &params, &sig));
        assert!(!twilio_signature_ok("wrong", url, &params, &sig));
    }

    #[test]
    fn previews_are_single_line_and_short() {
        assert_eq!(preview("hi\n\nthere"), "hi there");
        assert_eq!(preview(&"x".repeat(200)).chars().count(), 120);
    }
}
