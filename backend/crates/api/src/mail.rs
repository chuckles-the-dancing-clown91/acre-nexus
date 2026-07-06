//! **Inbound email routing** (issue #62) — the receiving half of a real email
//! story (outbound templated sends live in [`crate::notify`]).
//!
//! Tenants point their ESP's inbound hook (Postmark inbound, SendGrid Inbound
//! Parse, SES receiving) at the platform's signature-verified webhook door:
//! `POST /webhooks/inbound_email?tenant=<slug>` with the raw body signed under
//! `webhook.inbound_email.secret`. The verified event rides the durable queue
//! as a `webhook_event` job and lands here, where the **to-address** routes
//! it:
//!
//! | local part | routed to |
//! | --- | --- |
//! | `ticket+<uuid>@…` | a comment on that maintenance ticket |
//! | `leasing@…` | a CRM [`entity::lead`] (created or updated by sender) |
//! | anything else | logged unmatched |
//!
//! Every message is logged to [`entity::inbound_email`] — the tenant's
//! inbound communication history — and audited. Per-tenant addresses are
//! `<local>@<tenant-slug>.<INBOUND_EMAIL_DOMAIN>` (default
//! `in.acrenexus.com`).

use crate::modules::JobOutcome;
use chrono::Utc;
use entity::prelude::{Lead, MaintenanceTicket};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde_json::json;
use uuid::Uuid;

/// The webhook provider key inbound mail arrives under.
pub const PROVIDER: &str = "inbound_email";

/// The platform's inbound-mail apex domain (per-tenant addresses hang off
/// `<slug>.` under it).
pub fn inbound_domain() -> String {
    std::env::var("INBOUND_EMAIL_DOMAIN").unwrap_or_else(|_| "in.acrenexus.com".into())
}

/// The reply-to address that threads into one ticket's timeline.
pub fn ticket_address(tenant_slug: &str, ticket_id: Uuid) -> String {
    format!(
        "ticket+{}@{}.{}",
        ticket_id.simple(),
        tenant_slug,
        inbound_domain()
    )
}

/// The tenant's monitored leasing inbox.
pub fn leasing_address(tenant_slug: &str) -> String {
    format!("leasing@{}.{}", tenant_slug, inbound_domain())
}

// ---------------------------------------------------------------------------
// Address parsing (pure, unit-tested)
// ---------------------------------------------------------------------------

/// Where a to-address routes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Ticket(Uuid),
    Leasing,
    Unmatched,
}

/// Extract the bare address from `Name <addr@host>` / `addr@host` forms.
pub fn bare_address(raw: &str) -> &str {
    let raw = raw.trim();
    match (raw.rfind('<'), raw.rfind('>')) {
        (Some(start), Some(end)) if start < end => raw[start + 1..end].trim(),
        _ => raw,
    }
}

/// Route a to-address by its local part. Host is not re-checked — delivery to
/// the tenant's inbound subdomain is what got the message here.
pub fn route_for(to: &str) -> Route {
    let addr = bare_address(to).to_lowercase();
    let Some((local, _host)) = addr.split_once('@') else {
        return Route::Unmatched;
    };
    if local == "leasing" {
        return Route::Leasing;
    }
    if let Some(id) = local.strip_prefix("ticket+") {
        if let Ok(uuid) = Uuid::parse_str(id) {
            return Route::Ticket(uuid);
        }
    }
    Route::Unmatched
}

/// Sender display name from `Name <addr@host>`, falling back to the mailbox
/// local part.
pub fn sender_name(from: &str) -> String {
    let from = from.trim();
    if let Some(start) = from.rfind('<') {
        let name = from[..start].trim().trim_matches('"');
        if !name.is_empty() {
            return name.to_string();
        }
    }
    bare_address(from)
        .split('@')
        .next()
        .unwrap_or("Unknown sender")
        .to_string()
}

/// Pull a normalized `{from, to, subject, text}` out of an inbound event,
/// accepting both our lowercase contract and Postmark-style capitalized keys.
pub fn normalize_event(event: &serde_json::Value) -> Option<(String, String, String, String)> {
    let get = |lower: &str, upper: &str| -> Option<String> {
        event
            .get(lower)
            .or_else(|| event.get(upper))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    let from = get("from", "From")?;
    let to = get("to", "To")?;
    let subject = get("subject", "Subject").unwrap_or_default();
    let text = get("text", "TextBody").unwrap_or_default();
    Some((from, to, subject, text))
}

// ---------------------------------------------------------------------------
// The webhook_event consumer
// ---------------------------------------------------------------------------

/// Consume one verified `inbound_email` webhook event: route it, log it,
/// audit it. `None` when the event belongs to a different provider.
pub async fn handle_webhook_event(
    db: &DatabaseConnection,
    job: &entity::background_job::Model,
) -> Option<JobOutcome> {
    let provider = job.payload.get("provider").and_then(|v| v.as_str())?;
    if provider != PROVIDER {
        return None;
    }
    let event = job.payload.get("event").cloned().unwrap_or(json!({}));
    let Some((from, to, subject, text)) = normalize_event(&event) else {
        return Some(JobOutcome::failed(
            "inbound email event missing from/to fields",
        ));
    };

    let tenant_id = job.tenant_id;
    let route = route_for(&to);
    let (routed, routed_id) = match &route {
        Route::Ticket(ticket_id) => {
            match thread_into_ticket(db, tenant_id, *ticket_id, &from, &subject, &text).await {
                Ok(Some(comment_id)) => ("ticket_comment", Some(comment_id)),
                Ok(None) => ("unmatched", None),
                Err(e) => {
                    return Some(JobOutcome::retry(
                        crate::providers::backoff(job.attempts),
                        format!("db error: {e}"),
                    ))
                }
            }
        }
        Route::Leasing => match upsert_lead(db, tenant_id, &from, &subject, &text).await {
            Ok(lead_id) => ("lead", Some(lead_id)),
            Err(e) => {
                return Some(JobOutcome::retry(
                    crate::providers::backoff(job.attempts),
                    format!("db error: {e}"),
                ))
            }
        },
        Route::Unmatched => ("unmatched", None),
    };

    // The comms log — every inbound message, wherever it landed.
    let log = entity::inbound_email::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        from_email: Set(bare_address(&from).to_string()),
        to_email: Set(bare_address(&to).to_string()),
        subject: Set(subject.clone()),
        body_text: Set(text.clone()),
        routed: Set(routed.to_string()),
        routed_id: Set(routed_id),
        created_at: Set(Utc::now().into()),
    };
    let log_id = match log.insert(db).await {
        Ok(row) => row.id,
        Err(e) => {
            return Some(JobOutcome::retry(
                crate::providers::backoff(job.attempts),
                format!("db error: {e}"),
            ))
        }
    };

    crate::audit::record(
        db,
        None,
        crate::audit::actions::EMAIL_INBOUND,
        Some("inbound_email"),
        Some(log_id.to_string()),
        Some(tenant_id),
        Some(json!({ "routed": routed, "routed_id": routed_id })),
    )
    .await;

    Some(JobOutcome::completed(json!({
        "provider": PROVIDER,
        "routed": routed,
        "routed_id": routed_id,
    })))
}

/// Append an inbound reply to a ticket's timeline. `Ok(None)` when the ticket
/// doesn't exist (logged unmatched rather than erroring — mail is external
/// input).
async fn thread_into_ticket(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    from: &str,
    subject: &str,
    text: &str,
) -> Result<Option<Uuid>, sea_orm::DbErr> {
    let ticket = MaintenanceTicket::find_by_id(ticket_id)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    let Some(ticket) = ticket else {
        return Ok(None);
    };
    let body = if text.trim().is_empty() {
        subject.to_string()
    } else {
        text.trim().to_string()
    };
    let comment = entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        ticket_id: Set(ticket.id),
        author_user_id: Set(None),
        kind: Set("comment".into()),
        body: Set(format!(
            "Email reply from {}:\n\n{}",
            sender_name(from),
            body
        )),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    crate::audit::record(
        db,
        None,
        crate::audit::actions::TICKET_COMMENT_ADD,
        Some("maintenance_ticket"),
        Some(ticket.id.to_string()),
        Some(tenant_id),
        Some(json!({ "comment_id": comment.id, "via": "inbound_email" })),
    )
    .await;
    Ok(Some(comment.id))
}

/// Create a lead for a first-time sender to the leasing inbox, or refresh the
/// existing one (latest message + bump; a closed lead reopens as `new`).
async fn upsert_lead(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    from: &str,
    subject: &str,
    text: &str,
) -> Result<Uuid, sea_orm::DbErr> {
    let email = bare_address(from).to_lowercase();
    let excerpt: String = text.trim().chars().take(500).collect();
    let last_message = if subject.is_empty() {
        excerpt.clone()
    } else {
        format!("{subject}\n\n{excerpt}")
    };
    let now = Utc::now();

    let existing = Lead::find()
        .filter(entity::lead::Column::TenantId.eq(tenant_id))
        .filter(entity::lead::Column::Email.eq(email.clone()))
        .one(db)
        .await?;
    let (lead_id, action) = match existing {
        Some(lead) => {
            let id = lead.id;
            let reopened = lead.status == "closed";
            let mut am: entity::lead::ActiveModel = lead.into();
            am.last_message = Set(Some(last_message));
            if reopened {
                am.status = Set("new".into());
            }
            am.updated_at = Set(now.into());
            am.update(db).await?;
            (id, crate::audit::actions::LEAD_UPDATE)
        }
        None => {
            let lead = entity::lead::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                name: Set(sender_name(from)),
                email: Set(email),
                phone: Set(None),
                source: Set("inbound_email".into()),
                status: Set("new".into()),
                notes: Set(None),
                last_message: Set(Some(last_message)),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(db)
            .await?;

            // New prospect — tell the leasing team.
            crate::notify::notify_staff(
                db,
                tenant_id,
                "application:read",
                "lead_received",
                json!({
                    "lead_name": lead.name,
                    "lead_email": lead.email,
                    "subject": if subject.is_empty() { "(no subject)" } else { subject },
                }),
                Some(("lead", lead.id)),
                "received",
                None,
            )
            .await;
            (lead.id, crate::audit::actions::LEAD_CREATE)
        }
    };

    crate::audit::record(
        db,
        None,
        action,
        Some("lead"),
        Some(lead_id.to_string()),
        Some(tenant_id),
        Some(json!({ "source": "inbound_email" })),
    )
    .await;
    Ok(lead_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_parse_out_of_display_forms() {
        assert_eq!(
            bare_address("Taylor Brooks <t@example.com>"),
            "t@example.com"
        );
        assert_eq!(bare_address("t@example.com"), "t@example.com");
        assert_eq!(bare_address("  <t@example.com>  "), "t@example.com");
        assert_eq!(
            sender_name("Taylor Brooks <t@example.com>"),
            "Taylor Brooks"
        );
        assert_eq!(sender_name("t.brooks@example.com"), "t.brooks");
        assert_eq!(
            sender_name("\"Brooks, Taylor\" <t@e.com>"),
            "Brooks, Taylor"
        );
    }

    #[test]
    fn routing_matches_ticket_and_leasing_locals() {
        let id = Uuid::from_u128(7);
        let to = format!("ticket+{}@northwind.in.acrenexus.com", id.simple());
        assert_eq!(route_for(&to), Route::Ticket(id));
        // Display form + case-insensitivity.
        let to = format!(
            "Support <TICKET+{}@northwind.in.acrenexus.com>",
            id.simple()
        );
        assert_eq!(route_for(&to), Route::Ticket(id));
        assert_eq!(
            route_for("leasing@northwind.in.acrenexus.com"),
            Route::Leasing
        );
        // Garbage ticket ids and unknown mailboxes fall through.
        assert_eq!(
            route_for("ticket+not-a-uuid@northwind.in.acrenexus.com"),
            Route::Unmatched
        );
        assert_eq!(
            route_for("info@northwind.in.acrenexus.com"),
            Route::Unmatched
        );
        assert_eq!(route_for("no-at-sign"), Route::Unmatched);
    }

    #[test]
    fn events_normalize_from_both_contracts() {
        let ours = serde_json::json!({
            "from": "a@b.com", "to": "leasing@x.in.acrenexus.com",
            "subject": "Hi", "text": "Body"
        });
        let (f, t, s, b) = normalize_event(&ours).unwrap();
        assert_eq!(
            (f.as_str(), t.as_str(), s.as_str(), b.as_str()),
            ("a@b.com", "leasing@x.in.acrenexus.com", "Hi", "Body")
        );

        let postmark = serde_json::json!({
            "From": "a@b.com", "To": "leasing@x.in.acrenexus.com",
            "Subject": "Hi", "TextBody": "Body"
        });
        assert!(normalize_event(&postmark).is_some());
        assert!(normalize_event(&serde_json::json!({ "to": "x" })).is_none());
    }

    #[test]
    fn tenant_addresses_compose() {
        let id = Uuid::from_u128(7);
        let addr = ticket_address("northwind", id);
        assert!(addr.starts_with("ticket+"));
        assert!(addr.contains("@northwind."));
        assert_eq!(route_for(&addr), Route::Ticket(id));
        assert_eq!(route_for(&leasing_address("northwind")), Route::Leasing);
    }
}
