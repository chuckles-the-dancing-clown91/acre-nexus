//! **Follow-ups the scheduler sends on its own**, from the hourly helpdesk
//! scan. Each goes out once (the notice log keeps the key) and only while
//! it still makes sense:
//!
//! - the resident, a day after their work order is finished and not yet
//!   rated: how did it go? (`ticket_rating_request`);
//! - the resident, a week after: is it still fixed? (`ticket_checkin`);
//! - a vendor sent tasks by email who hasn't answered from their link
//!   (`vendor_task_nudge`, with a fresh link);
//! - the person offered visit times who hasn't picked, while the times are
//!   still ahead (`appointment_offer_reminder`, with a fresh link);
//! - a prospect who toured and hasn't applied (`lead_after_showing`).
//!
//! Each has a setting under "Follow-ups" (hours or days; 0 turns it off).

use crate::appointments as appt;
use chrono::{Duration, Utc};
use entity::prelude::{Appointment, Counterparty, Lead, MaintenanceTicket, Tenant, TicketTask};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, Set,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

/// Someone outside the team: an email, a number, or both.
struct Person<'a> {
    email: Option<&'a str>,
    phone: Option<&'a str>,
}

/// Email, and a text when there's a number, to someone outside the team.
async fn send_both(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    to: Person<'_>,
    template: &str,
    vars: Value,
    owner: (&str, Uuid),
    trigger: &str,
) {
    if let Some(e) = to.email.map(str::trim).filter(|e| !e.is_empty()) {
        crate::notify::notify_person(
            db,
            tenant_id,
            e,
            template,
            vars.clone(),
            Some(owner),
            trigger,
        )
        .await;
    }
    if let Some(p) = to.phone.map(str::trim).filter(|p| !p.is_empty()) {
        let sms = json!({
            "template": template,
            "to": p,
            "owner_type": owner.0,
            "owner_id": owner.1,
            "trigger": format!("{trigger}:sms"),
            "vars": vars,
        });
        if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_sms", sms, 0).await {
            tracing::error!("follow-up sms: {e}");
        }
    }
}

async fn company(db: &impl ConnectionTrait, tenant_id: Uuid) -> (String, String) {
    Tenant::find_by_id(tenant_id)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|t| (t.name, t.slug))
        .unwrap_or_default()
}

fn request_link(ticket_id: Uuid) -> String {
    format!(
        "{}/account/maintenance?ticket={}",
        crate::resident_reminders::web_url(),
        ticket_id
    )
}

/// Residents whose finished work orders want a word: a rating ask after
/// `rating_hours`, a check-in after `checkin_days`.
async fn resident_followups(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    rating_hours: i64,
    checkin_days: i64,
) -> Result<(u32, u32), DbErr> {
    if rating_hours <= 0 && checkin_days <= 0 {
        return Ok((0, 0));
    }
    let now = Utc::now();
    // Anything finished in the last 60 days; older ones have had their say.
    let finished = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::Status.is_in(["resolved", "closed"]))
        .filter(entity::maintenance_ticket::Column::ResolvedAt.is_not_null())
        .filter(entity::maintenance_ticket::Column::ResolvedAt.gte(now - Duration::days(60)))
        .all(db)
        .await?;
    let (company_name, _) = company(db, tenant_id).await;
    let (mut ratings, mut checkins) = (0u32, 0u32);
    for t in finished {
        let Some(resolved) = t.resolved_at.map(|r| r.with_timezone(&Utc)) else {
            continue;
        };
        let Some(lease) = appt::resident_for_ticket(db, tenant_id, &t).await else {
            continue;
        };
        if lease.tenant_email.is_none() && lease.tenant_phone.is_none() {
            continue;
        }
        let vars = json!({
            "name": lease.tenant_name,
            "title": t.title,
            "company": company_name,
            "link": request_link(t.id),
        });
        if rating_hours > 0
            && t.rating.is_none()
            && resolved <= now - Duration::hours(rating_hours)
            && crate::notices::claim(db, tenant_id, &format!("followup:rating:{}", t.id)).await?
        {
            send_both(
                db,
                tenant_id,
                Person {
                    email: lease.tenant_email.as_deref(),
                    phone: lease.tenant_phone.as_deref(),
                },
                "ticket_rating_request",
                vars.clone(),
                ("maintenance_ticket", t.id),
                &format!("followup_rating:{}", t.id),
            )
            .await;
            ratings += 1;
        }
        if checkin_days > 0
            && resolved <= now - Duration::days(checkin_days)
            && crate::notices::claim(db, tenant_id, &format!("followup:checkin:{}", t.id)).await?
        {
            send_both(
                db,
                tenant_id,
                Person {
                    email: lease.tenant_email.as_deref(),
                    phone: lease.tenant_phone.as_deref(),
                },
                "ticket_checkin",
                vars,
                ("maintenance_ticket", t.id),
                &format!("followup_checkin:{}", t.id),
            )
            .await;
            checkins += 1;
        }
    }
    Ok((ratings, checkins))
}

/// Vendors sent tasks by email who haven't answered: one nudge per batch,
/// with a fresh link (the old one still works until they use either).
async fn vendor_nudges(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    hours: i64,
) -> Result<u32, DbErr> {
    if hours <= 0 {
        return Ok(0);
    }
    let now = Utc::now();
    let quiet = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_task::Column::AssigneeEntityId.is_not_null())
        .filter(entity::ticket_task::Column::DispatchVia.eq("email"))
        .filter(entity::ticket_task::Column::VendorTokenHash.is_not_null())
        .filter(entity::ticket_task::Column::VendorResponse.is_null())
        .filter(entity::ticket_task::Column::Status.is_not_in(["done", "skipped"]))
        .filter(entity::ticket_task::Column::DispatchedAt.lte(now - Duration::hours(hours)))
        .filter(entity::ticket_task::Column::DispatchedAt.gte(now - Duration::days(30)))
        .all(db)
        .await?;
    // A batch is the tasks sharing one link.
    let mut batches: HashMap<String, Vec<entity::ticket_task::Model>> = HashMap::new();
    for t in quiet {
        if let Some(h) = t.vendor_token_hash.clone() {
            batches.entry(h).or_default().push(t);
        }
    }
    let (company_name, _) = company(db, tenant_id).await;
    let mut sent = 0u32;
    for (_, mut tasks) in batches {
        // The batch is remembered by its tasks, not its link: the nudge mints
        // a new link, and the batch mustn't come round again.
        tasks.sort_by_key(|t| t.id);
        let first = &tasks[0];
        let short = format!("{}:{}", first.ticket_id, first.id);
        if !crate::notices::claim(db, tenant_id, &format!("followup:vendor:{short}")).await? {
            continue;
        }
        let Some(vendor) = first
            .assignee_entity_id
            .map(Counterparty::find_by_id)
            .map(|q| q.one(db))
        else {
            continue;
        };
        let Some(vendor) = vendor.await? else {
            continue;
        };
        let Some(email) = vendor.email.clone().filter(|e| !e.trim().is_empty()) else {
            continue;
        };
        let Some(ticket) = MaintenanceTicket::find_by_id(first.ticket_id)
            .one(db)
            .await?
        else {
            continue;
        };
        if !crate::routes::maintenance::is_open(&ticket.status) {
            continue;
        }
        let token = crate::auth::random_secret(24);
        for t in &tasks {
            let mut am: entity::ticket_task::ActiveModel = t.clone().into();
            am.vendor_token_hash = Set(Some(crate::auth::hash_secret(&token)));
            am.updated_at = Set(now.into());
            am.update(db).await?;
        }
        let lines = tasks
            .iter()
            .map(|t| format!("- {}", t.title))
            .collect::<Vec<_>>()
            .join("\n");
        let days = (now
            - first
                .dispatched_at
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or(now))
        .num_days()
        .max(1);
        send_both(
            db,
            tenant_id,
            Person {
                email: Some(&email),
                phone: vendor.phone.as_deref(),
            },
            "vendor_task_nudge",
            json!({
                "vendor": vendor.name,
                "title": ticket.title,
                "count": tasks.len(),
                "tasks": lines,
                "days": days,
                "company": company_name,
                "vendor_link": crate::routes::maintenance::vendor_link::url(&token),
            }),
            ("maintenance_ticket", ticket.id),
            &format!("followup_vendor:{short}"),
        )
        .await;
        appt::note_on_ticket(
            db,
            tenant_id,
            ticket.id,
            "vendor_nudged",
            &format!(
                "Nudged {} about {} task{} with no answer after {} day{}.",
                vendor.name,
                tasks.len(),
                if tasks.len() == 1 { "" } else { "s" },
                days,
                if days == 1 { "" } else { "s" }
            ),
            "internal",
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

/// Offered visit times nobody has picked yet, while the times are still
/// ahead: send the link again.
async fn offer_reminders(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    hours: i64,
) -> Result<u32, DbErr> {
    if hours <= 0 {
        return Ok(0);
    }
    let now = Utc::now();
    let waiting = Appointment::find()
        .filter(entity::appointment::Column::TenantId.eq(tenant_id))
        .filter(entity::appointment::Column::Status.eq("proposed"))
        .filter(entity::appointment::Column::TokenHash.is_not_null())
        .filter(entity::appointment::Column::CreatedAt.lte(now - Duration::hours(hours)))
        .filter(entity::appointment::Column::CreatedAt.gte(now - Duration::days(30)))
        .all(db)
        .await?;
    let mut sent = 0u32;
    for a in waiting {
        let still_ahead = appt::windows_of(&a.windows)
            .iter()
            .any(|w| w.start > now + Duration::hours(1));
        if !still_ahead {
            continue;
        }
        if !crate::notices::claim(db, tenant_id, &format!("followup:offer:{}", a.id)).await? {
            continue;
        }
        if let Err(e) = appt::resend_offer(db, tenant_id, a).await {
            tracing::error!("offer reminder: {e}");
            continue;
        }
        sent += 1;
    }
    Ok(sent)
}

/// Prospects who toured and haven't applied: a nudge with the application
/// link, once.
async fn prospect_followups(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    days: i64,
) -> Result<u32, DbErr> {
    if days <= 0 {
        return Ok(0);
    }
    let now = Utc::now();
    let toured = Lead::find()
        .filter(entity::lead::Column::TenantId.eq(tenant_id))
        .filter(entity::lead::Column::Status.eq("toured"))
        .filter(entity::lead::Column::ApplicationId.is_null())
        .filter(entity::lead::Column::UpdatedAt.lte(now - Duration::days(days)))
        .filter(entity::lead::Column::UpdatedAt.gte(now - Duration::days(45)))
        .all(db)
        .await?;
    let (company_name, slug) = company(db, tenant_id).await;
    let mut sent = 0u32;
    for l in toured {
        if l.email.trim().is_empty() && l.phone.as_deref().unwrap_or("").trim().is_empty() {
            continue;
        }
        if !crate::notices::claim(db, tenant_id, &format!("followup:lead:{}", l.id)).await? {
            continue;
        }
        send_both(
            db,
            tenant_id,
            Person {
                email: Some(&l.email),
                phone: l.phone.as_deref(),
            },
            "lead_after_showing",
            json!({
                "name": l.name,
                "company": company_name,
                "link": crate::routes::leads::invite::apply_url(&slug, &l, None),
            }),
            ("lead", l.id),
            &format!("followup_lead:{}", l.id),
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

/// Run every follow-up for a workspace; the counts go on the scan summary.
pub async fn run(db: &impl ConnectionTrait, tenant_id: Uuid) -> Result<Value, DbErr> {
    use crate::settings::{self, get_i64};
    let (ratings, checkins) = resident_followups(
        db,
        tenant_id,
        get_i64(db, tenant_id, settings::FOLLOWUPS_RATING_HOURS).await,
        get_i64(db, tenant_id, settings::FOLLOWUPS_CHECKIN_DAYS).await,
    )
    .await?;
    let vendors = vendor_nudges(
        db,
        tenant_id,
        get_i64(db, tenant_id, settings::FOLLOWUPS_VENDOR_HOURS).await,
    )
    .await?;
    let offers = offer_reminders(
        db,
        tenant_id,
        get_i64(db, tenant_id, settings::FOLLOWUPS_OFFER_HOURS).await,
    )
    .await?;
    let prospects = prospect_followups(
        db,
        tenant_id,
        get_i64(db, tenant_id, settings::FOLLOWUPS_PROSPECT_DAYS).await,
    )
    .await?;
    Ok(json!({
        "rating_requests": ratings,
        "checkins": checkins,
        "vendor_nudges": vendors,
        "offer_reminders": offers,
        "prospect_nudges": prospects,
    }))
}
