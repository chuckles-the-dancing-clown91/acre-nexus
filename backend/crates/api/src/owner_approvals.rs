//! **Owner approvals and sign-off.** Work estimated over the owner's limit
//! waits for their yes before it goes to a vendor; finished billable work
//! asks them to sign off. Owners answer from a link in the email or text, or
//! from their portal. Pending asks are nudged, and the monthly statement goes
//! out from here too.

use crate::error::{ApiError, ApiResult};
use crate::routes::maintenance::desk::cost_summary;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use entity::prelude::{
    EntityOwnership, Llc, MaintenanceTicket, Owner, OwnerApproval, Property, Tenant,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::json;
use uuid::Uuid;

/// Where an approval link points.
pub fn link(token: &str) -> String {
    format!("{}/approve/{token}", crate::oauth::public_app_url())
}

/// `$1,850` or `$1,850.50`.
pub fn money(cents: i64) -> String {
    if cents % 100 == 0 {
        crate::dto::usd(cents)
    } else {
        format!(
            "{}.{:02}",
            crate::dto::usd(cents - cents.rem_euclid(100)),
            cents.rem_euclid(100)
        )
    }
}

/// The owner behind a property: the biggest stake in its LLC.
pub async fn owner_for_property(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    property_id: Uuid,
) -> ApiResult<Option<entity::owner::Model>> {
    let Some(p) = Property::find_by_id(property_id)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    let Some(llc) = p.llc_id else { return Ok(None) };
    let stake = EntityOwnership::find()
        .filter(entity::entity_ownership::Column::TenantId.eq(tenant_id))
        .filter(entity::entity_ownership::Column::EntityId.eq(llc))
        .order_by_desc(entity::entity_ownership::Column::OwnershipBps)
        .one(db)
        .await?;
    match stake {
        Some(s) => Ok(Owner::find_by_id(s.owner_id)
            .filter(entity::owner::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?),
        None => Ok(None),
    }
}

/// The owner a signed-in user is: linked by `user_id`, else by email.
pub async fn owner_for_user(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    user_id: Uuid,
    email: &str,
) -> ApiResult<Option<entity::owner::Model>> {
    if let Some(o) = Owner::find()
        .filter(entity::owner::Column::TenantId.eq(tenant_id))
        .filter(entity::owner::Column::UserId.eq(user_id))
        .one(db)
        .await?
    {
        return Ok(Some(o));
    }
    let email = email.trim().to_lowercase();
    if email.is_empty() {
        return Ok(None);
    }
    Ok(Owner::find()
        .filter(entity::owner::Column::TenantId.eq(tenant_id))
        .filter(entity::owner::Column::Email.eq(email))
        .one(db)
        .await?)
}

/// The LLCs and properties an owner holds.
pub async fn holdings(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    owner_id: Uuid,
) -> ApiResult<(Vec<entity::llc::Model>, Vec<entity::property::Model>)> {
    let llc_ids: Vec<Uuid> = EntityOwnership::find()
        .filter(entity::entity_ownership::Column::TenantId.eq(tenant_id))
        .filter(entity::entity_ownership::Column::OwnerId.eq(owner_id))
        .all(db)
        .await?
        .into_iter()
        .map(|s| s.entity_id)
        .collect();
    if llc_ids.is_empty() {
        return Ok((vec![], vec![]));
    }
    let llcs = Llc::find()
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .filter(entity::llc::Column::Id.is_in(llc_ids.clone()))
        .all(db)
        .await?;
    let props = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .filter(entity::property::Column::LlcId.is_in(llc_ids))
        .order_by_asc(entity::property::Column::Name)
        .all(db)
        .await?;
    Ok((llcs, props))
}

/// The owner's limit: their own, else the workspace's. 0 means never ask.
pub async fn limit_for(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    owner: &entity::owner::Model,
) -> i64 {
    match owner.approval_limit_cents {
        Some(l) => l,
        None => {
            crate::settings::get_i64(
                db,
                tenant_id,
                crate::settings::MAINTENANCE_OWNER_APPROVAL_CENTS,
            )
            .await
        }
    }
}

async fn property_words(db: &impl ConnectionTrait, property_id: Uuid) -> String {
    Property::find_by_id(property_id)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|p| format!("{}, {}", p.address, p.city))
        .unwrap_or_default()
}

/// Email and text the owner about an ask.
async fn tell_owner(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    owner: &entity::owner::Model,
    template: &str,
    vars: serde_json::Value,
    approval_id: Uuid,
    trigger: &str,
) {
    if let Some(email) = owner.email.as_deref().filter(|e| !e.trim().is_empty()) {
        crate::notify::notify_person(
            db,
            tenant_id,
            email,
            template,
            vars.clone(),
            Some(("owner_approval", approval_id)),
            trigger,
        )
        .await;
    }
    if let Some(phone) = owner.phone.as_deref().filter(|p| !p.trim().is_empty()) {
        let sms = json!({
            "template": template,
            "to": phone,
            "owner_type": "owner_approval",
            "owner_id": approval_id,
            "trigger": format!("{trigger}:sms"),
            "vars": vars,
        });
        if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_sms", sms, 0).await {
            tracing::error!("owner sms: {e}");
        }
    }
}

/// A line on the work order in the owner's name (or the system's).
async fn note_on_ticket(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    author: &str,
    action: &str,
    body: &str,
    visibility: &str,
) {
    let c = entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        ticket_id: Set(ticket_id),
        task_id: Set(None),
        author_user_id: Set(None),
        kind: Set("action".into()),
        visibility: Set(visibility.into()),
        author_name: Set(Some(author.to_string())),
        body: Set(body.to_string()),
        document_ids: Set(json!([])),
        action: Set(Some(action.into())),
        created_at: Set(Utc::now().into()),
    };
    if let Err(e) = c.insert(db).await {
        tracing::error!("owner approval note: {e}");
    }
}

/// The latest ask of a kind on a work order.
pub async fn latest(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    kind: &str,
) -> ApiResult<Option<entity::owner_approval::Model>> {
    Ok(OwnerApproval::find()
        .filter(entity::owner_approval::Column::TenantId.eq(tenant_id))
        .filter(entity::owner_approval::Column::TicketId.eq(ticket_id))
        .filter(entity::owner_approval::Column::Kind.eq(kind))
        .order_by_desc(entity::owner_approval::Column::RequestedAt)
        .one(db)
        .await?)
}

/// What an ask is about.
struct Ask<'a> {
    owner: &'a entity::owner::Model,
    ticket_id: Uuid,
    kind: &'a str,
    amount_cents: i64,
    note: Option<String>,
    requested_by: Option<Uuid>,
    status: &'a str,
    override_reason: Option<String>,
}

async fn insert_ask(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    a: Ask<'_>,
) -> ApiResult<(entity::owner_approval::Model, String)> {
    let token = crate::auth::random_secret(24);
    let now = Utc::now();
    let overridden = a.status == "overridden";
    let saved = entity::owner_approval::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        owner_id: Set(a.owner.id),
        ticket_id: Set(a.ticket_id),
        kind: Set(a.kind.into()),
        amount_cents: Set(a.amount_cents),
        status: Set(a.status.into()),
        token_hash: Set(Some(crate::auth::hash_secret(&token))),
        note: Set(a.note),
        requested_by: Set(a.requested_by),
        requested_at: Set(now.into()),
        decided_at: Set(if overridden { Some(now.into()) } else { None }),
        decided_by: Set(if overridden {
            Some("staff".into())
        } else {
            None
        }),
        decision_note: Set(None),
        override_reason: Set(a.override_reason),
        nudged_at: Set(None),
        nudges: Set(0),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok((saved, token))
}

/// Put the work order on hold for the owner, with a follow-up in three days.
async fn hold_for_owner(
    db: &impl ConnectionTrait,
    t: &entity::maintenance_ticket::Model,
) -> ApiResult<()> {
    if !matches!(
        t.status.as_str(),
        "open" | "triage" | "scheduled" | "on_hold"
    ) {
        return Ok(());
    }
    let mut am: entity::maintenance_ticket::ActiveModel = t.clone().into();
    am.status = Set("on_hold".into());
    am.waiting_on = Set(Some("owner".into()));
    am.follow_up_date = Set(Some(
        (Utc::now().date_naive() + Duration::days(3)).to_string(),
    ));
    am.updated_at = Set(Utc::now().into());
    am.update(db).await?;
    Ok(())
}

/// Send the owner the ask for approval.
pub async fn request(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    t: &entity::maintenance_ticket::Model,
    owner: &entity::owner::Model,
    amount_cents: i64,
    note: Option<String>,
    requested_by: Option<Uuid>,
) -> ApiResult<entity::owner_approval::Model> {
    let limit = limit_for(db, tenant_id, owner).await;
    let (saved, token) = insert_ask(
        db,
        tenant_id,
        Ask {
            owner,
            ticket_id: t.id,
            kind: "approval",
            amount_cents,
            note: note.clone(),
            requested_by,
            status: "pending",
            override_reason: None,
        },
    )
    .await?;
    hold_for_owner(db, t).await?;
    let property = property_words(db, t.property_id).await;
    tell_owner(
        db,
        tenant_id,
        owner,
        "owner_approval_request",
        json!({
            "title": t.title,
            "property": property,
            "amount": money(amount_cents),
            "limit": money(limit),
            "note": note.map(|n| format!("\n\n\"{n}\"")).unwrap_or_default(),
            "link": link(&token),
        }),
        saved.id,
        &format!("owner_approval:{}", saved.id),
    )
    .await;
    note_on_ticket(
        db,
        tenant_id,
        t.id,
        "Owner approvals",
        "owner_approval_requested",
        &format!(
            "Asked {} to approve {} of work (over their {} limit).",
            owner.name,
            money(amount_cents),
            money(limit)
        ),
        "internal",
    )
    .await;
    Ok(saved)
}

/// Before work goes out: under the limit, or approved, it passes. Over it
/// with no approval, this returns a conflict that says so and changes
/// nothing (the request rolls back with it); staff ask the owner from the
/// work order, or give a reason to go ahead, which is recorded on the ask.
pub async fn require_approval(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    t: &entity::maintenance_ticket::Model,
    by: Option<Uuid>,
    override_reason: Option<&str>,
) -> ApiResult<Option<entity::owner_approval::Model>> {
    let Some(owner) = owner_for_property(db, tenant_id, t.property_id).await? else {
        return Ok(None);
    };
    let limit = limit_for(db, tenant_id, &owner).await;
    if limit <= 0 {
        return Ok(None);
    }
    let est = cost_summary(db, tenant_id, t).await?.est_total_cents;
    if est < limit {
        return Ok(None);
    }
    let existing = latest(db, tenant_id, t.id, "approval").await?;
    if let Some(a) = &existing {
        if matches!(a.status.as_str(), "approved" | "overridden") {
            return Ok(existing);
        }
    }
    let reason = override_reason
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(str::to_string);
    if let Some(reason) = reason {
        let (saved, _) = insert_ask(
            db,
            tenant_id,
            Ask {
                owner: &owner,
                ticket_id: t.id,
                kind: "approval",
                amount_cents: est,
                note: None,
                requested_by: by,
                status: "overridden",
                override_reason: Some(reason.clone()),
            },
        )
        .await?;
        note_on_ticket(
            db,
            tenant_id,
            t.id,
            "Owner approvals",
            "owner_approval_overridden",
            &format!(
                "Went ahead with {} of work without {}'s approval: \"{reason}\"",
                money(est),
                owner.name
            ),
            "internal",
        )
        .await;
        crate::audit::record(
            db,
            by,
            crate::audit::actions::OWNER_APPROVAL_OVERRIDE,
            Some("maintenance_ticket"),
            Some(t.id.to_string()),
            Some(tenant_id),
            Some(json!({ "amount_cents": est, "owner_id": owner.id, "reason": reason })),
        )
        .await;
        return Ok(Some(saved));
    }
    match existing.as_ref().map(|a| a.status.as_str()) {
        Some("pending") => Err(ApiError::Conflict(format!(
            "Waiting on {}'s approval of {} (asked {}). Add a reason to go ahead without it.",
            owner.name,
            money(existing.as_ref().unwrap().amount_cents),
            existing.as_ref().unwrap().requested_at.format("%b %-d")
        ))),
        Some("declined") => Err(ApiError::Conflict(format!(
            "{} declined {} of work on this. Change the plan, ask again, or add a reason to go ahead.",
            owner.name,
            money(existing.as_ref().unwrap().amount_cents)
        ))),
        _ => Err(ApiError::Conflict(format!(
            "{} of work is over {}'s {} limit. Ask them to approve it from the work order, or add a reason to go ahead.",
            money(est),
            owner.name,
            money(limit)
        ))),
    }
}

/// Finished billable work asks the owner to sign off (when the workspace
/// wants that and the owner is known).
pub async fn request_signoff(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    t: &entity::maintenance_ticket::Model,
    by: Option<Uuid>,
) -> ApiResult<Option<entity::owner_approval::Model>> {
    if !crate::settings::get_bool(db, tenant_id, crate::settings::MAINTENANCE_OWNER_SIGNOFF).await {
        return Ok(None);
    }
    let Some(owner) = owner_for_property(db, tenant_id, t.property_id).await? else {
        return Ok(None);
    };
    let actual = cost_summary(db, tenant_id, t).await?.actual_total_cents;
    if actual <= 0 {
        return Ok(None);
    }
    if let Some(a) = latest(db, tenant_id, t.id, "signoff").await? {
        if a.status == "pending" || a.status == "approved" {
            return Ok(Some(a));
        }
    }
    let (saved, token) = insert_ask(
        db,
        tenant_id,
        Ask {
            owner: &owner,
            ticket_id: t.id,
            kind: "signoff",
            amount_cents: actual,
            note: None,
            requested_by: by,
            status: "pending",
            override_reason: None,
        },
    )
    .await?;
    let property = property_words(db, t.property_id).await;
    tell_owner(
        db,
        tenant_id,
        &owner,
        "owner_signoff_request",
        json!({
            "title": t.title,
            "property": property,
            "amount": money(actual),
            "note": "",
            "link": link(&token),
        }),
        saved.id,
        &format!("owner_signoff:{}", saved.id),
    )
    .await;
    note_on_ticket(
        db,
        tenant_id,
        t.id,
        "Owner approvals",
        "owner_signoff_requested",
        &format!(
            "Asked {} to sign off on {} of finished work.",
            owner.name,
            money(actual)
        ),
        "internal",
    )
    .await;
    Ok(Some(saved))
}

/// The owner (or staff for them) answers an ask.
pub async fn decide(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    a: entity::owner_approval::Model,
    approve: bool,
    note: Option<String>,
    by: &str,
) -> ApiResult<entity::owner_approval::Model> {
    if a.status != "pending" {
        return Err(ApiError::Conflict(format!("this was already {}", a.status)));
    }
    let now = Utc::now();
    let status = match (a.kind.as_str(), approve) {
        (_, true) => "approved",
        ("signoff", false) => "disputed",
        _ => "declined",
    };
    let note = note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    let mut am: entity::owner_approval::ActiveModel = a.clone().into();
    am.status = Set(status.into());
    am.decided_at = Set(Some(now.into()));
    am.decided_by = Set(Some(by.into()));
    am.decision_note = Set(note.clone());
    am.updated_at = Set(now.into());
    let saved = am.update(db).await?;
    let owner = Owner::find_by_id(a.owner_id).one(db).await?;
    let owner_name = owner
        .as_ref()
        .map(|o| o.name.clone())
        .unwrap_or_else(|| "The owner".into());
    let Some(t) = MaintenanceTicket::find_by_id(a.ticket_id).one(db).await? else {
        return Ok(saved);
    };
    // The work order follows the answer.
    let mut tm: entity::maintenance_ticket::ActiveModel = t.clone().into();
    let mut changed = false;
    match (a.kind.as_str(), status) {
        ("approval", "approved") => {
            if t.status == "on_hold" && t.waiting_on.as_deref() == Some("owner") {
                tm.status = Set("open".into());
                tm.waiting_on = Set(None);
                tm.follow_up_date = Set(None);
                changed = true;
            }
        }
        ("approval", _) => {
            if t.waiting_on.as_deref() == Some("owner") {
                tm.waiting_on = Set(Some("other".into()));
                changed = true;
            }
        }
        ("signoff", "approved") => {
            if t.status == "resolved" {
                tm.status = Set("closed".into());
                changed = true;
            }
        }
        ("signoff", _) => {
            tm.status = Set("on_hold".into());
            tm.waiting_on = Set(Some("owner".into()));
            tm.follow_up_date = Set(Some((now.date_naive() + Duration::days(2)).to_string()));
            tm.resolved_at = Set(None);
            changed = true;
        }
        _ => {}
    }
    if changed {
        tm.updated_at = Set(now.into());
        tm.update(db).await?;
    }
    let line = match (a.kind.as_str(), status) {
        ("approval", "approved") => {
            format!("{owner_name} approved {} of work.", money(a.amount_cents))
        }
        ("approval", _) => format!("{owner_name} declined {} of work.", money(a.amount_cents)),
        ("signoff", "approved") => format!(
            "{owner_name} signed off on the work ({}).",
            money(a.amount_cents)
        ),
        _ => format!(
            "{owner_name} disputed the finished work ({}).",
            money(a.amount_cents)
        ),
    };
    let line = match &note {
        Some(n) => format!("{line} \"{n}\""),
        None => line,
    };
    note_on_ticket(
        db,
        tenant_id,
        t.id,
        &owner_name,
        &format!("owner_{status}"),
        &line,
        "internal",
    )
    .await;
    let decision = match (a.kind.as_str(), status) {
        ("approval", "approved") => "approved",
        ("approval", _) => "declined",
        ("signoff", "approved") => "signed off on",
        _ => "disputed",
    };
    crate::notify::notify_staff(
        db,
        tenant_id,
        "maintenance:manage",
        "owner_approval_decided",
        json!({
            "owner": owner_name,
            "decision": decision,
            "title": t.title,
            "property": property_words(db, t.property_id).await,
            "amount": money(a.amount_cents),
            "note": note.map(|n| format!("\n\n\"{n}\"")).unwrap_or_default(),
        }),
        Some(("maintenance_ticket", t.id)),
        &format!("owner_decided:{}:{}", a.id, now.timestamp()),
        None,
    )
    .await;
    Ok(saved)
}

/// The ask a link opens.
pub async fn by_token(
    db: &impl ConnectionTrait,
    token: &str,
) -> ApiResult<entity::owner_approval::Model> {
    let token = token.trim();
    if token.len() < 16 {
        return Err(ApiError::NotFound("that link isn't valid".into()));
    }
    OwnerApproval::find()
        .filter(entity::owner_approval::Column::TokenHash.eq(crate::auth::hash_secret(token)))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("that link isn't valid".into()))
}

/// Remind owners of asks waiting two days or more, every two days, three
/// times at most. Returns how many went out.
pub async fn nudge_pending(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<u32> {
    let now = Utc::now();
    let pending = OwnerApproval::find()
        .filter(entity::owner_approval::Column::TenantId.eq(tenant_id))
        .filter(entity::owner_approval::Column::Status.eq("pending"))
        .all(db)
        .await?;
    let mut sent = 0;
    for a in pending {
        if a.nudges >= 3 {
            continue;
        }
        let last = a
            .nudged_at
            .map(|d| d.to_utc())
            .unwrap_or(a.requested_at.to_utc());
        if now - last < Duration::hours(48) {
            continue;
        }
        let Some(owner) = Owner::find_by_id(a.owner_id).one(db).await? else {
            continue;
        };
        let Some(t) = MaintenanceTicket::find_by_id(a.ticket_id).one(db).await? else {
            continue;
        };
        if a.kind == "approval" && !matches!(t.status.as_str(), "on_hold" | "open" | "triage") {
            continue;
        }
        // The link is minted once; the reminder re-sends the same page by a
        // fresh token so the old one keeps working too.
        let token = crate::auth::random_secret(24);
        let template = if a.kind == "approval" {
            "owner_approval_reminder"
        } else {
            "owner_signoff_request"
        };
        let mut am: entity::owner_approval::ActiveModel = a.clone().into();
        am.nudged_at = Set(Some(now.into()));
        am.nudges = Set(a.nudges + 1);
        am.token_hash = Set(Some(crate::auth::hash_secret(&token)));
        am.updated_at = Set(now.into());
        am.update(db).await?;
        tell_owner(
            db,
            tenant_id,
            &owner,
            template,
            json!({
                "title": t.title,
                "property": property_words(db, t.property_id).await,
                "amount": money(a.amount_cents),
                "note": "",
                "link": link(&token),
            }),
            a.id,
            &format!("owner_nudge:{}:{}", a.id, a.nudges + 1),
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

/// One owner's month: every LLC's rent, expenses and fee, plus the work done.
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct Statement {
    pub owner_id: Uuid,
    pub owner_name: String,
    /// `YYYY-MM`.
    pub month: String,
    pub period_start: String,
    pub period_end: String,
    pub entities: Vec<crate::routes::reports::owner_statement::OwnerStatementResp>,
    pub rent_collected_cents: i64,
    pub expenses_cents: i64,
    pub mgmt_fee_cents: i64,
    pub net_cents: i64,
    pub rent_collected_label: String,
    pub expenses_label: String,
    pub mgmt_fee_label: String,
    pub net_label: String,
    pub work: Vec<WorkDone>,
    pub approvals: Vec<ApprovalLine>,
}

#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct WorkDone {
    pub ticket_id: Uuid,
    pub title: String,
    pub property: String,
    pub resolved_on: String,
    pub cost_cents: i64,
    pub cost_label: String,
    pub status: String,
}

#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct ApprovalLine {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub status: String,
    pub decided_at: Option<String>,
}

/// First and last day of `YYYY-MM`.
pub fn month_bounds(month: &str) -> Option<(NaiveDate, NaiveDate)> {
    let (y, m) = month.split_once('-')?;
    let (y, m) = (y.parse::<i32>().ok()?, m.parse::<u32>().ok()?);
    let start = NaiveDate::from_ymd_opt(y, m, 1)?;
    let next = if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1)?
    };
    Some((start, next.pred_opt()?))
}

/// "October 2026".
pub fn month_words(month: &str) -> String {
    match month_bounds(month) {
        Some((d, _)) => d.format("%B %Y").to_string(),
        None => month.to_string(),
    }
}

/// Build an owner's statement for a month.
pub async fn statement(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    owner: &entity::owner::Model,
    month: &str,
) -> ApiResult<Statement> {
    let (start, end) =
        month_bounds(month).ok_or_else(|| ApiError::BadRequest("month must be YYYY-MM".into()))?;
    let (llcs, props) = holdings(db, tenant_id, owner.id).await?;
    let mut entities = vec![];
    for l in &llcs {
        match crate::routes::reports::owner_statement::build(
            db,
            tenant_id,
            l.id,
            &start.to_string(),
            &end.to_string(),
        )
        .await
        {
            Ok(s) => entities.push(s),
            Err(e) => tracing::warn!("owner statement for {}: {e}", l.name),
        }
    }
    let rent: i64 = entities.iter().map(|e| e.rent_collected_cents).sum();
    let expenses: i64 = entities.iter().map(|e| e.expenses_cents).sum();
    let fee: i64 = entities.iter().map(|e| e.mgmt_fee_cents).sum();
    let net: i64 = entities.iter().map(|e| e.net_cents).sum();
    let prop_ids: Vec<Uuid> = props.iter().map(|p| p.id).collect();
    let mut work = vec![];
    if !prop_ids.is_empty() {
        let start_ts = start.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_ts = (end + Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let tickets = MaintenanceTicket::find()
            .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
            .filter(entity::maintenance_ticket::Column::PropertyId.is_in(prop_ids.clone()))
            .filter(entity::maintenance_ticket::Column::ResolvedAt.gte(start_ts))
            .filter(entity::maintenance_ticket::Column::ResolvedAt.lt(end_ts))
            .order_by_desc(entity::maintenance_ticket::Column::ResolvedAt)
            .all(db)
            .await?;
        for t in tickets {
            let cost = cost_summary(db, tenant_id, &t).await?.actual_total_cents;
            work.push(WorkDone {
                ticket_id: t.id,
                title: t.title.clone(),
                property: props
                    .iter()
                    .find(|p| p.id == t.property_id)
                    .map(|p| p.name.clone())
                    .unwrap_or_default(),
                resolved_on: t
                    .resolved_at
                    .map(|d| d.date_naive().to_string())
                    .unwrap_or_default(),
                cost_cents: cost,
                cost_label: money(cost),
                status: t.status.clone(),
            });
        }
    }
    let approvals = OwnerApproval::find()
        .filter(entity::owner_approval::Column::TenantId.eq(tenant_id))
        .filter(entity::owner_approval::Column::OwnerId.eq(owner.id))
        .order_by_desc(entity::owner_approval::Column::RequestedAt)
        .all(db)
        .await?;
    let mut lines = vec![];
    for a in approvals {
        let d = a.requested_at.date_naive();
        if d < start || d > end {
            continue;
        }
        let title = MaintenanceTicket::find_by_id(a.ticket_id)
            .one(db)
            .await?
            .map(|t| t.title)
            .unwrap_or_default();
        lines.push(ApprovalLine {
            id: a.id,
            kind: a.kind,
            title,
            amount_label: money(a.amount_cents),
            amount_cents: a.amount_cents,
            status: a.status,
            decided_at: a.decided_at.map(|d| d.to_rfc3339()),
        });
    }
    Ok(Statement {
        owner_id: owner.id,
        owner_name: owner.name.clone(),
        month: month.to_string(),
        period_start: start.to_string(),
        period_end: end.to_string(),
        entities,
        rent_collected_cents: rent,
        expenses_cents: expenses,
        mgmt_fee_cents: fee,
        net_cents: net,
        rent_collected_label: money(rent),
        expenses_label: money(expenses),
        mgmt_fee_label: money(fee),
        net_label: money(net),
        work,
        approvals: lines,
    })
}

/// The statement as a PDF.
pub fn statement_pdf(s: &Statement, company: &str) -> Vec<u8> {
    use crate::pdfdoc::{Block, Document, Table};
    let mut blocks = vec![Block::KeyValues(vec![
        ("Owner".into(), s.owner_name.clone()),
        (
            "Period".into(),
            format!("{} to {}", s.period_start, s.period_end),
        ),
        ("Rent collected".into(), s.rent_collected_label.clone()),
        ("Expenses".into(), s.expenses_label.clone()),
        ("Management fee".into(), s.mgmt_fee_label.clone()),
        ("Net to you".into(), s.net_label.clone()),
    ])];
    for e in &s.entities {
        blocks.push(Block::Heading(e.entity_name.clone()));
        let mut rows: Vec<Vec<String>> = vec![vec![
            "Rent collected".into(),
            e.rent_collected_label.clone(),
        ]];
        for l in &e.expense_lines {
            rows.push(vec![l.name.clone(), format!("-{}", l.amount_label)]);
        }
        rows.push(vec![
            "Management fee".into(),
            format!("-{}", e.mgmt_fee_label),
        ]);
        blocks.push(Block::Table(Table::auto(
            &["Item".into(), "Amount".into()],
            rows,
            Some(vec!["Net".into(), e.net_label.clone()]),
        )));
    }
    if !s.work.is_empty() {
        blocks.push(Block::Heading("Work done this month".into()));
        let rows = s
            .work
            .iter()
            .map(|w| {
                vec![
                    w.resolved_on.clone(),
                    w.property.clone(),
                    w.title.clone(),
                    w.cost_label.clone(),
                ]
            })
            .collect();
        blocks.push(Block::Table(Table::auto(
            &[
                "Done".into(),
                "Property".into(),
                "Work".into(),
                "Cost".into(),
            ],
            rows,
            None,
        )));
    }
    if !s.approvals.is_empty() {
        blocks.push(Block::Heading("Your approvals".into()));
        let rows = s
            .approvals
            .iter()
            .map(|a| {
                vec![
                    a.title.clone(),
                    if a.kind == "approval" {
                        "Approval".into()
                    } else {
                        "Sign-off".into()
                    },
                    a.amount_label.clone(),
                    a.status.clone(),
                ]
            })
            .collect();
        blocks.push(Block::Table(Table::auto(
            &[
                "Work".into(),
                "Ask".into(),
                "Amount".into(),
                "Answer".into(),
            ],
            rows,
            None,
        )));
    }
    crate::pdfdoc::render(&Document {
        title: format!("Owner statement, {}", month_words(&s.month)),
        subtitle: Some(s.owner_name.clone()),
        organization: company.to_string(),
        landscape: false,
        blocks,
    })
}

/// On the statement day, email every owner last month's statement, once.
pub async fn send_statements_if_due(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<u32> {
    let day = crate::settings::get_i64(db, tenant_id, crate::settings::OWNERS_STATEMENT_DAY).await;
    if day <= 0 {
        return Ok(0);
    }
    let tz = crate::appointments::tz_for(db, tenant_id).await;
    let today = Utc::now().with_timezone(&tz).date_naive();
    if (today.day() as i64) < day {
        return Ok(0);
    }
    let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
    let last_month = first.pred_opt().unwrap();
    let month = last_month.format("%Y-%m").to_string();
    let company = Tenant::find_by_id(tenant_id)
        .one(db)
        .await?
        .map(|t| t.name)
        .unwrap_or_default();
    let owners = Owner::find()
        .filter(entity::owner::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let mut sent = 0;
    for o in owners {
        let Some(email) = o.email.as_deref().filter(|e| !e.trim().is_empty()) else {
            continue;
        };
        let (_, props) = holdings(db, tenant_id, o.id).await?;
        if props.is_empty() {
            continue;
        }
        if !crate::notices::claim(db, tenant_id, &format!("owner_statement:{}:{month}", o.id))
            .await?
        {
            continue;
        }
        let s = statement(db, tenant_id, &o, &month).await?;
        let _ = company.as_str();
        crate::notify::notify_person(
            db,
            tenant_id,
            email,
            "owner_statement",
            json!({
                "month": month_words(&month),
                "rent": s.rent_collected_label,
                "expenses": s.expenses_label,
                "net": s.net_label,
                "link": format!("{}/account/owner/statement?month={month}", crate::oauth::public_app_url()),
            }),
            Some(("owner", o.id)),
            &format!("owner_statement:{month}"),
        )
        .await;
        sent += 1;
    }
    Ok(sent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_have_bounds_and_names() {
        let (s, e) = month_bounds("2026-02").unwrap();
        assert_eq!(s.to_string(), "2026-02-01");
        assert_eq!(e.to_string(), "2026-02-28");
        let (_, e) = month_bounds("2026-12").unwrap();
        assert_eq!(e.to_string(), "2026-12-31");
        assert!(month_bounds("2026").is_none());
        assert_eq!(month_words("2026-10"), "October 2026");
    }

    #[test]
    fn money_keeps_cents_only_when_needed() {
        assert_eq!(money(185000), "$1,850");
        assert_eq!(money(185050), "$1,850.50");
    }
}
