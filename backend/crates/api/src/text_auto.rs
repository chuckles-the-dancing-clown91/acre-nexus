//! What happens on its own around texts (fix plan F14–F17):
//!
//! * **Rate by text.** Resolving a resident's work order asks "how did we do?"
//!   by text (or by email when there's no number). A lone 1–5 from that
//!   resident within [`RATING_WINDOW_DAYS`] becomes the ticket's review.
//! * **Text to work order.** A resident's text that reads like a repair gets a
//!   link to a request already filled in, at most once a day per thread.
//! * **Quiet hours.** Automatic texts wait for the morning, in the workspace's
//!   time zone. Staff-typed replies and replies to someone who just texted go
//!   at once.
//! * **Photos by text.** MMS media are fetched from Twilio and filed as
//!   documents on the resident's lease.

use crate::modules::JobOutcome;
use crate::notices;
use crate::settings as cfg;
use crate::texts;
use chrono::{Duration, NaiveTime, Timelike, Utc};
use entity::prelude::{Lease, MaintenanceTicket, NoticeLog, SmsMessage, SmsThread};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, QueryOrder, Set,
};
use serde_json::json;
use uuid::Uuid;

/// How long after the ask a bare digit still counts as a rating.
pub const RATING_WINDOW_DAYS: i64 = 7;
/// Largest photo we'll file from a text (Twilio caps MMS at 5 MB).
const MAX_MEDIA_BYTES: usize = 10 * 1024 * 1024;

/// Templates that go out at any hour: answers to something the person just
/// did, and security messages they are waiting on.
pub const QUIET_EXEMPT: &[&str] = &[
    "direct_text",
    "password_reset",
    "account_invite",
    "test_notification",
    "ticket_rating_thanks",
    "repair_link",
];

// ---------------------------------------------------------------------------
// Pure rules
// ---------------------------------------------------------------------------

/// A text that is only a 1–5 rating: "5", "4!", "3 stars", "5/5".
pub fn rating_digit(body: &str) -> Option<i32> {
    let t = body.trim().to_lowercase();
    let t = t.trim_end_matches(['.', '!', '?', ' ']);
    let t = t
        .strip_suffix("/5")
        .or_else(|| t.strip_suffix(" stars"))
        .or_else(|| t.strip_suffix(" star"))
        .unwrap_or(t)
        .trim();
    match t {
        "1" | "2" | "3" | "4" | "5" => t.parse().ok(),
        _ => None,
    }
}

/// Words that mean something in the home needs fixing, by category.
const REPAIR_WORDS: &[(&str, &[&str])] = &[
    (
        "plumbing",
        &[
            "leak", "leaking", "leaks", "drip", "dripping", "clog", "clogged", "toilet", "sink",
            "faucet", "drain", "pipe", "shower", "tub", "flood", "flooding",
        ],
    ),
    (
        "electrical",
        &[
            "outlet",
            "breaker",
            "sparking",
            "sparks",
            "lights",
            "light",
            "power",
            "switch",
            "electrical",
        ],
    ),
    (
        "hvac",
        &[
            "heat",
            "heater",
            "furnace",
            "ac",
            "a/c",
            "thermostat",
            "cooling",
            "heating",
        ],
    ),
    (
        "appliance",
        &[
            "fridge",
            "refrigerator",
            "dishwasher",
            "washer",
            "dryer",
            "oven",
            "stove",
            "microwave",
            "disposal",
            "freezer",
        ],
    ),
    (
        "structural",
        &["roof", "ceiling", "crack", "mold", "window", "door", "lock"],
    ),
];

/// Words that turn a household noun into a repair ("the sink" is not one,
/// "the sink is broken" is).
const TROUBLE_WORDS: &[&str] = &[
    "broken", "broke", "leak", "leaking", "leaks", "not", "isn't", "isnt", "won't", "wont",
    "doesn't", "doesnt", "stopped", "clogged", "clog", "drip", "dripping", "flood", "flooding",
    "sparking", "sparks", "out", "stuck", "cracked", "crack", "mold", "no", "fix", "repair",
    "busted", "loud", "smell", "smells",
];

fn words(body: &str) -> Vec<String> {
    body.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '/'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Whether a text reads like a repair, and if so a title and category for
/// the request. A catalog issue whose words all appear wins the title.
pub fn repair_guess(body: &str, catalog: &[(String, String)]) -> Option<(String, String)> {
    let ws = words(body);
    if ws.len() < 2 {
        return None;
    }
    let category = REPAIR_WORDS
        .iter()
        .find(|(_, list)| ws.iter().any(|w| list.contains(&w.as_str())))
        .map(|(c, _)| c.to_string())?;
    if !ws.iter().any(|w| TROUBLE_WORDS.contains(&w.as_str())) {
        return None;
    }
    for (name, cat) in catalog {
        let need: Vec<String> = words(name).into_iter().filter(|w| w.len() > 3).collect();
        if !need.is_empty()
            && need.iter().all(|n| {
                ws.iter()
                    .any(|w| w == n || w.starts_with(n.trim_end_matches("ing")))
            })
        {
            return Some((name.clone(), cat.clone()));
        }
    }
    let title: String = texts::preview(body).chars().take(60).collect();
    Some((title, category))
}

/// Seconds until quiet hours end, or `None` when texts may go now. Quiet
/// hours run from `start` to `end` o'clock and may wrap past midnight.
pub fn quiet_delay(now: NaiveTime, start: u32, end: u32) -> Option<i64> {
    if start == end || start > 23 || end > 23 {
        return None;
    }
    let h = now.hour();
    let quiet = if start > end {
        h >= start || h < end
    } else {
        h >= start && h < end
    };
    if !quiet {
        return None;
    }
    let end_t = NaiveTime::from_hms_opt(end, 0, 0)?;
    let mut secs = (end_t - now).num_seconds();
    if secs <= 0 {
        secs += 24 * 3600;
    }
    Some(secs)
}

/// The link to a prefilled maintenance request in the resident portal.
pub fn request_link(title: &str, category: &str, description: &str) -> String {
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    (b as char).to_string()
                }
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    let description: String = description.chars().take(500).collect();
    format!(
        "{}/account/maintenance?new=1&title={}&category={}&description={}",
        crate::resident_reminders::web_url(),
        enc(title),
        enc(category),
        enc(&description)
    )
}

// ---------------------------------------------------------------------------
// Sending
// ---------------------------------------------------------------------------

/// Queue an automatic text to `phone` (STOP and quiet hours are applied when
/// the job runs).
pub async fn send_auto(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    phone: &str,
    template: &str,
    vars: serde_json::Value,
    owner: (&str, Uuid),
    trigger: &str,
) {
    let payload = json!({
        "template": template,
        "to": phone,
        "vars": vars,
        "owner_type": owner.0,
        "owner_id": owner.1.to_string(),
        "trigger": trigger,
    });
    if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_sms", payload, 0).await {
        tracing::error!("failed to enqueue {template} text: {e}");
    }
}

/// How long an automatic text must wait for quiet hours to end, if at all.
pub async fn quiet_wait(db: &impl ConnectionTrait, tenant_id: Uuid) -> Option<i64> {
    if !cfg::get_bool(db, tenant_id, cfg::TEXTS_QUIET_HOURS).await {
        return None;
    }
    let tz = cfg::get_string(db, tenant_id, cfg::TEXTS_TIMEZONE)
        .await
        .parse::<chrono_tz::Tz>()
        .unwrap_or(chrono_tz::America::Los_Angeles);
    let start = cfg::get_i64(db, tenant_id, cfg::TEXTS_QUIET_START).await;
    let end = cfg::get_i64(db, tenant_id, cfg::TEXTS_QUIET_END).await;
    let local = Utc::now().with_timezone(&tz).time();
    quiet_delay(local, start.clamp(0, 23) as u32, end.clamp(0, 23) as u32)
}

// ---------------------------------------------------------------------------
// F14: rate by text
// ---------------------------------------------------------------------------

fn ask_key(ticket_id: Uuid) -> String {
    format!("rate_ask:{ticket_id}")
}

/// Ask the resident how the repair went, once per work order.
pub async fn ask_for_rating(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket: &entity::maintenance_ticket::Model,
) -> Result<(), DbErr> {
    if ticket.rating.is_some() || !cfg::get_bool(db, tenant_id, cfg::MAINTENANCE_ASK_RATING).await {
        return Ok(());
    }
    let Some(lease_id) = ticket.lease_id else {
        return Ok(());
    };
    let Some(lease) = Lease::find_by_id(lease_id)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
    else {
        return Ok(());
    };
    let phone = lease
        .tenant_phone
        .as_deref()
        .and_then(texts::normalize_phone);
    let email = lease
        .tenant_email
        .as_deref()
        .map(str::trim)
        .filter(|e| !e.is_empty());
    if phone.is_none() && email.is_none() {
        return Ok(());
    }
    if !notices::claim(db, tenant_id, &ask_key(ticket.id)).await? {
        return Ok(());
    }
    let vars = json!({
        "title": ticket.title,
        "url": format!("{}/account/maintenance?ticket={}", crate::resident_reminders::web_url(), ticket.id),
    });
    match phone {
        Some(p) if !texts::is_opted_out(db, tenant_id, &p).await => {
            send_auto(
                db,
                tenant_id,
                &p,
                "ticket_rate_request",
                vars,
                ("maintenance_ticket", ticket.id),
                "rate_ask",
            )
            .await;
        }
        _ => {
            if let Some(e) = email {
                crate::notify::notify_person(
                    db,
                    tenant_id,
                    e,
                    "ticket_rate_request",
                    vars,
                    Some(("maintenance_ticket", ticket.id)),
                    "rate_ask",
                )
                .await;
            }
        }
    }
    Ok(())
}

/// A bare 1–5 from a resident we asked: store it as the review. Returns
/// whether the text was taken as a rating.
pub async fn take_rating(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    thread: &entity::sms_thread::Model,
    body: &str,
) -> Result<bool, DbErr> {
    let Some(rating) = rating_digit(body) else {
        return Ok(false);
    };
    let Some(lease_id) = thread.lease_id else {
        return Ok(false);
    };
    let since = Utc::now() - Duration::days(RATING_WINDOW_DAYS);
    let candidates = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::LeaseId.eq(lease_id))
        .filter(entity::maintenance_ticket::Column::Rating.is_null())
        .filter(entity::maintenance_ticket::Column::Status.is_in(["resolved", "closed"]))
        .filter(entity::maintenance_ticket::Column::ResolvedAt.gte(since))
        .order_by_desc(entity::maintenance_ticket::Column::ResolvedAt)
        .all(db)
        .await?;
    let mut ticket = None;
    for t in candidates {
        let asked = NoticeLog::find()
            .filter(entity::notice_log::Column::TenantId.eq(tenant_id))
            .filter(entity::notice_log::Column::Key.eq(ask_key(t.id)))
            .one(db)
            .await?
            .is_some();
        if asked {
            ticket = Some(t);
            break;
        }
    }
    let Some(ticket) = ticket else {
        return Ok(false);
    };
    let now = Utc::now();
    let done = MaintenanceTicket::update_many()
        .col_expr(
            entity::maintenance_ticket::Column::Rating,
            Expr::value(rating),
        )
        .col_expr(
            entity::maintenance_ticket::Column::ReviewedAt,
            Expr::value(now),
        )
        .col_expr(
            entity::maintenance_ticket::Column::UpdatedAt,
            Expr::value(now),
        )
        .filter(entity::maintenance_ticket::Column::Id.eq(ticket.id))
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::Rating.is_null())
        .exec(db)
        .await?;
    if done.rows_affected == 0 {
        return Ok(false);
    }
    crate::audit::record(
        db,
        None,
        crate::audit::actions::TICKET_REVIEW,
        Some("maintenance_ticket"),
        Some(ticket.id.to_string()),
        Some(tenant_id),
        Some(json!({ "rating": rating, "via": "text" })),
    )
    .await;
    let resident = thread
        .display_name
        .clone()
        .unwrap_or_else(|| thread.phone.clone());
    crate::notify::notify_staff(
        db,
        tenant_id,
        "maintenance:read",
        "ticket_reviewed",
        json!({
            "title": ticket.title,
            "resident": resident,
            "rating": rating,
            "stars": "★".repeat(rating as usize),
            "comment": "(rated by text)",
        }),
        Some(("maintenance_ticket", ticket.id)),
        "reviewed",
        None,
    )
    .await;
    send_auto(
        db,
        tenant_id,
        &thread.phone,
        "ticket_rating_thanks",
        json!({ "title": ticket.title, "rating": rating }),
        ("maintenance_ticket", ticket.id),
        "rated",
    )
    .await;
    Ok(true)
}

// ---------------------------------------------------------------------------
// F15: text to work order
// ---------------------------------------------------------------------------

/// Answer a resident's repair-sounding text with a prefilled request link.
pub async fn offer_repair_link(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    thread: &entity::sms_thread::Model,
    body: &str,
) -> Result<bool, DbErr> {
    if thread.lease_id.is_none() || !cfg::get_bool(db, tenant_id, cfg::TEXTS_REPAIR_LINKS).await {
        return Ok(false);
    }
    let catalog: Vec<(String, String)> = entity::prelude::IssueTemplate::find()
        .filter(entity::issue_template::Column::TenantId.eq(tenant_id))
        .filter(entity::issue_template::Column::Active.eq(true))
        .all(db)
        .await?
        .into_iter()
        .map(|t| (t.name, t.category))
        .collect();
    let Some((title, category)) = repair_guess(body, &catalog) else {
        return Ok(false);
    };
    let today = Utc::now().date_naive();
    if !notices::claim(db, tenant_id, &format!("repair_link:{}:{today}", thread.id)).await? {
        return Ok(false);
    }
    send_auto(
        db,
        tenant_id,
        &thread.phone,
        "repair_link",
        json!({ "url": request_link(&title, &category, body), "title": title }),
        ("sms_thread", thread.id),
        "repair_link",
    )
    .await;
    Ok(true)
}

/// Everything automatic after a text comes in. Best-effort: a failure here
/// never loses the text itself.
pub async fn after_inbound(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    thread: &entity::sms_thread::Model,
    body: &str,
) {
    if thread.opted_out_at.is_some() {
        return;
    }
    match take_rating(db, tenant_id, thread, body).await {
        Ok(true) => return,
        Ok(false) => {}
        Err(e) => tracing::error!("text rating failed: {e}"),
    }
    if let Err(e) = offer_repair_link(db, tenant_id, thread, body).await {
        tracing::error!("repair link failed: {e}");
    }
}

// ---------------------------------------------------------------------------
// F16: photos by text
// ---------------------------------------------------------------------------

/// Fetch a text's photos from Twilio and file them on the resident's lease
/// (or the conversation, for a number we don't know).
pub async fn handle_media_job(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> JobOutcome {
    let tenant_id = job.tenant_id;
    let Some(mid) = job.payload["message_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
    else {
        return JobOutcome::failed("sms_media payload missing message_id");
    };
    let Ok(Some(message)) = SmsMessage::find_by_id(mid).one(db).await else {
        return JobOutcome::failed("text not found");
    };
    if !crate::providers::is_live("sms") {
        return JobOutcome::completed(json!({ "skipped": true, "reason": "texts are simulated" }));
    }
    let Some(provider) = crate::notify::default_provider(db, tenant_id, "sms")
        .await
        .filter(|p| p.kind == "twilio")
    else {
        return JobOutcome::completed(json!({ "skipped": true, "reason": "no Twilio provider" }));
    };
    let sid = provider.config["account_sid"].as_str().unwrap_or_default();
    let token = match provider.secret_ref.as_deref() {
        Some(k) => crate::providers::ProviderCtx::new(db, tenant_id)
            .secret(k)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let Some(token) = token.filter(|_| !sid.is_empty()) else {
        return JobOutcome::failed("Twilio credentials are incomplete");
    };
    let thread = SmsThread::find_by_id(message.thread_id)
        .one(db)
        .await
        .ok()
        .flatten();
    let (owner_type, owner_id) = match thread.as_ref().and_then(|t| t.lease_id) {
        Some(l) => ("lease", l),
        None => ("sms_thread", message.thread_id),
    };
    let store = match crate::storage::ObjectStore::from_env() {
        Ok(s) => s,
        Err(e) => return JobOutcome::failed(format!("object store: {e}")),
    };
    let client = reqwest::Client::new();
    let mut filed = vec![];
    let items = job.payload["media"].as_array().cloned().unwrap_or_default();
    for (n, item) in items.iter().enumerate() {
        let Some(url) = item["url"].as_str() else {
            continue;
        };
        // Only ever send our Twilio credentials to Twilio.
        if !url.starts_with("https://api.twilio.com/") {
            continue;
        }
        let resp = match client.get(url).basic_auth(sid, Some(&token)).send().await {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => return JobOutcome::failed(format!("media fetch: HTTP {}", r.status())),
            Err(e) => return JobOutcome::failed(format!("media fetch: {e}")),
        };
        let content_type = item["content_type"]
            .as_str()
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = match resp.bytes().await {
            Ok(b) if b.len() <= MAX_MEDIA_BYTES => b,
            Ok(_) => continue,
            Err(e) => return JobOutcome::failed(format!("media read: {e}")),
        };
        let id = Uuid::new_v4();
        let key = format!("{tenant_id}/{id}");
        if let Err(e) = store.put_bytes(&key, &bytes).await {
            return JobOutcome::failed(format!("store: {e}"));
        }
        let ext = content_type.rsplit('/').next().unwrap_or("bin");
        let now = Utc::now();
        let doc = entity::document::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            owner_type: Set(owner_type.into()),
            owner_id: Set(owner_id),
            filename: Set(format!(
                "text-photo-{}-{}.{ext}",
                message.created_at.format("%Y%m%d-%H%M"),
                n + 1
            )),
            category: Set(Some("other".into())),
            requires_wet_ink: Set(false),
            physical_location: Set(None),
            mime_type: Set(content_type.clone()),
            size_bytes: Set(bytes.len() as i64),
            checksum: Set(Some(crate::storage::sha256_hex(&bytes))),
            version: Set(1),
            previous_version_id: Set(None),
            storage_key: Set(key),
            status: Set("stored".into()),
            retention_expires_at: Set(None),
            created_by: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };
        if let Err(e) = doc.insert(db).await {
            return JobOutcome::failed(format!("document: {e}"));
        }
        filed.push(json!({ "document_id": id, "content_type": content_type }));
    }
    let count = filed.len();
    let mut am: entity::sms_message::ActiveModel = message.into();
    am.media = Set(json!(filed));
    if let Err(e) = am.update(db).await {
        return JobOutcome::failed(format!("text update: {e}"));
    }
    JobOutcome::completed(json!({ "filed": count }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratings_are_bare_digits() {
        assert_eq!(rating_digit("5"), Some(5));
        assert_eq!(rating_digit(" 4! "), Some(4));
        assert_eq!(rating_digit("3 stars"), Some(3));
        assert_eq!(rating_digit("5/5"), Some(5));
        assert_eq!(rating_digit("0"), None);
        assert_eq!(rating_digit("6"), None);
        assert_eq!(rating_digit("unit 5"), None);
        assert_eq!(rating_digit("10"), None);
    }

    #[test]
    fn repairs_are_recognised() {
        let catalog = vec![("Leaking faucet".to_string(), "plumbing".to_string())];
        assert_eq!(
            repair_guess("my kitchen faucet is leaking again", &catalog),
            Some(("Leaking faucet".into(), "plumbing".into()))
        );
        let (_, cat) = repair_guess("the dishwasher won't drain", &[]).unwrap();
        assert_eq!(cat, "plumbing", "drain is the first category hit");
        let (_, cat) = repair_guess("heater is broken, no heat", &[]).unwrap();
        assert_eq!(cat, "hvac");
        assert_eq!(repair_guess("thanks, see you at the door", &[]), None);
        assert_eq!(repair_guess("ok", &[]), None);
        assert_eq!(repair_guess("when is rent due?", &[]), None);
    }

    #[test]
    fn quiet_hours_wrap_midnight() {
        let t = |h, m| NaiveTime::from_hms_opt(h, m, 0).unwrap();
        assert_eq!(quiet_delay(t(12, 0), 21, 8), None);
        assert_eq!(quiet_delay(t(21, 0), 21, 8), Some(11 * 3600));
        assert_eq!(quiet_delay(t(7, 30), 21, 8), Some(30 * 60));
        assert_eq!(quiet_delay(t(8, 0), 21, 8), None);
        assert_eq!(quiet_delay(t(2, 0), 1, 5), Some(3 * 3600));
        assert_eq!(quiet_delay(t(6, 0), 1, 5), None);
        assert_eq!(quiet_delay(t(3, 0), 8, 8), None, "start = end means off");
    }

    #[test]
    fn request_links_are_encoded() {
        let l = request_link("Leaking faucet", "plumbing", "sink & tap drip?");
        assert!(l.contains("/account/maintenance?new=1&title=Leaking%20faucet&category=plumbing"));
        assert!(l.contains("description=sink%20%26%20tap%20drip%3F"));
    }
}
