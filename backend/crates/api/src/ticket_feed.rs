//! The work order **feed**: one stream of everything that happened on it,
//! newest first. Notes, status moves, button presses, photos and video,
//! expenses, time logged, tasks finished, and what vendors said, all in one
//! place. A task's own feed is the same stream narrowed to that task.
//!
//! Also the database side of the status rules in [`crate::ticket_flow`]: the
//! facts a move is judged on, and stopping clocks when work ends.

use crate::error::ApiResult;
use crate::ticket_flow::Facts;
use crate::workforce::{self, entry_minutes, Rules};
use chrono::{DateTime, Utc};
use entity::prelude::{Appointment, Document, Expense, TicketComment, TicketTask, TimeEntry, User};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

/// A file shown in the feed.
#[derive(Serialize, schemars::JsonSchema, Clone)]
pub struct FeedFile {
    pub id: Uuid,
    pub filename: String,
    /// `photo` | `video` | `receipt` | `document`.
    pub kind: String,
    pub mime_type: String,
    pub url: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct FeedItem {
    pub id: String,
    pub at: String,
    /// `note` | `resident` | `status` | `action` | `photo` | `video` |
    /// `file` | `expense` | `time` | `task` | `vendor`.
    pub kind: String,
    pub actor: Option<String>,
    pub title: String,
    pub body: Option<String>,
    /// `public` (the resident sees it) | `internal`.
    pub visibility: String,
    pub amount_cents: Option<i64>,
    pub minutes: Option<i64>,
    /// A time entry still running.
    pub running: bool,
    pub task_id: Option<Uuid>,
    pub task_title: Option<String>,
    pub files: Vec<FeedFile>,
}

fn file_kind(category: Option<&str>) -> &'static str {
    match category {
        Some("photo") => "photo",
        Some("video") => "video",
        Some("receipt") => "receipt",
        _ => "document",
    }
}

fn item(id: String, at: DateTime<Utc>, kind: &str, title: String) -> FeedItem {
    FeedItem {
        id,
        at: at.to_rfc3339(),
        kind: kind.into(),
        actor: None,
        title,
        body: None,
        visibility: "internal".into(),
        amount_cents: None,
        minutes: None,
        running: false,
        task_id: None,
        task_title: None,
        files: vec![],
    }
}

/// Everything on a work order, newest first; `task` narrows it to one task's
/// notes, answers and progress.
pub async fn feed(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
    task: Option<Uuid>,
    sign: &(dyn Fn(&str) -> Option<String> + Sync),
) -> ApiResult<Vec<FeedItem>> {
    let comments = TicketComment::find()
        .filter(entity::ticket_comment::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_comment::Column::TicketId.eq(ticket_id))
        .all(db)
        .await?;
    let tasks: HashMap<Uuid, entity::ticket_task::Model> = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_task::Column::TicketId.eq(ticket_id))
        .all(db)
        .await?
        .into_iter()
        .map(|t| (t.id, t))
        .collect();
    let docs: HashMap<Uuid, entity::document::Model> = Document::find()
        .filter(entity::document::Column::TenantId.eq(tenant_id))
        .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
        .filter(entity::document::Column::OwnerId.eq(ticket_id))
        .all(db)
        .await?
        .into_iter()
        .map(|d| (d.id, d))
        .collect();
    let file = |id: &Uuid| {
        docs.get(id).map(|d| FeedFile {
            id: d.id,
            filename: d.filename.clone(),
            kind: file_kind(d.category.as_deref()).into(),
            mime_type: d.mime_type.clone(),
            url: sign(&d.storage_key),
        })
    };
    let mut names: HashMap<Uuid, String> = HashMap::new();
    let who = |ids: Vec<Uuid>| async move {
        let mut out: HashMap<Uuid, String> = HashMap::new();
        if ids.is_empty() {
            return out;
        }
        if let Ok(users) = User::find()
            .filter(entity::user::Column::Id.is_in(ids))
            .all(db)
            .await
        {
            for u in users {
                out.insert(
                    u.id,
                    if u.name.trim().is_empty() {
                        u.email
                    } else {
                        u.name
                    },
                );
            }
        }
        out
    };

    let entries = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::MaintenanceTicketId.eq(ticket_id))
        .all(db)
        .await?;
    let expenses = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(tenant_id))
        .filter(entity::expense::Column::MaintenanceTicketId.eq(ticket_id))
        .all(db)
        .await?;
    let mut ids: Vec<Uuid> = entries.iter().map(|e| e.user_id).collect();
    ids.extend(tasks.values().filter_map(|t| t.done_by));
    ids.extend(expenses.iter().filter_map(|e| e.recorded_by));
    names.extend(who(ids).await);
    let name_of = |id: Option<Uuid>| id.and_then(|i| names.get(&i).cloned());

    let mut out: Vec<FeedItem> = vec![];
    let mut attached: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

    for c in &comments {
        if task.is_some() && c.task_id != task {
            continue;
        }
        let docs_in: Vec<Uuid> = serde_json::from_value(c.document_ids.clone()).unwrap_or_default();
        attached.extend(docs_in.iter().copied());
        let resident = c.action.as_deref() == Some("resident_comment");
        let kind = if resident {
            "resident"
        } else {
            match c.kind.as_str() {
                "status" => "status",
                "action" => "action",
                _ => "note",
            }
        };
        let title = match kind {
            "status" | "action" => c.body.clone(),
            _ => String::new(),
        };
        let mut it = item(format!("c-{}", c.id), c.created_at.to_utc(), kind, title);
        it.body = (!matches!(kind, "status" | "action")).then(|| c.body.clone());
        it.actor = c.author_name.clone();
        it.visibility = c.visibility.clone();
        it.task_id = c.task_id;
        it.task_title = c
            .task_id
            .and_then(|t| tasks.get(&t).map(|x| x.title.clone()));
        it.files = docs_in.iter().filter_map(&file).collect();
        out.push(it);
    }

    if task.is_none() {
        // Photos and video not already on a note stand on their own.
        for d in docs.values() {
            if attached.contains(&d.id) {
                continue;
            }
            let kind = match file_kind(d.category.as_deref()) {
                "photo" => "photo",
                "video" => "video",
                "receipt" => continue,
                _ => "file",
            };
            let mut it = item(
                format!("d-{}", d.id),
                d.created_at.to_utc(),
                kind,
                d.filename.clone(),
            );
            it.files = file(&d.id).into_iter().collect();
            out.push(it);
        }
        for e in &expenses {
            let receipts: Vec<Uuid> = serde_json::from_value(
                e.details
                    .get("receipt_document_ids")
                    .cloned()
                    .unwrap_or(serde_json::json!([])),
            )
            .unwrap_or_default();
            let mut it = item(
                format!("x-{}", e.id),
                e.created_at.to_utc(),
                "expense",
                e.description.clone(),
            );
            it.actor = name_of(e.recorded_by);
            it.body = e.vendor.clone();
            it.amount_cents = Some(e.amount_cents);
            it.visibility = if e.billable_to_owner {
                "public"
            } else {
                "internal"
            }
            .into();
            it.files = receipts.iter().filter_map(&file).collect();
            out.push(it);
        }
        let now = Utc::now();
        for e in &entries {
            let running = e.ended_at.is_none();
            let mins = entry_minutes(e, now);
            let mut it = item(
                format!("t-{}", e.id),
                e.started_at.to_utc(),
                "time",
                if running {
                    "Clocked in".into()
                } else {
                    "Time logged".into()
                },
            );
            it.actor = name_of(Some(e.user_id));
            it.body = e.notes.clone();
            it.minutes = Some(mins);
            it.running = running;
            out.push(it);
        }
    }

    for t in tasks.values() {
        if task.is_some() && task != Some(t.id) {
            continue;
        }
        let base = |suffix: &str, at: DateTime<Utc>, kind: &str, title: String| {
            let mut it = item(format!("k-{}-{suffix}", t.id), at, kind, title);
            it.task_id = Some(t.id);
            it.task_title = Some(t.title.clone());
            it
        };
        if let (Some(at), "done") = (t.done_at, t.status.as_str()) {
            let mut it = base(
                "done",
                at.to_utc(),
                "task",
                format!("Task done: {}", t.title),
            );
            it.actor = name_of(t.done_by);
            out.push(it);
        }
        if let Some(at) = t.dispatched_at {
            let mut it = base(
                "sent",
                at.to_utc(),
                "vendor",
                format!("Sent to a vendor: {}", t.title),
            );
            it.body = t.dispatch_note.clone();
            out.push(it);
        }
        if let (Some(at), Some(r)) = (t.vendor_responded_at, t.vendor_response.as_deref()) {
            let words = match r {
                "accepted" => "The vendor accepted",
                "declined" => "The vendor declined",
                _ => "The vendor finished",
            };
            let mut it = base(
                "answer",
                at.to_utc(),
                "vendor",
                format!("{words}: {}", t.title),
            );
            it.body = t.vendor_note.clone();
            it.visibility = "internal".into();
            out.push(it);
        }
    }

    out.sort_by(|a, b| b.at.cmp(&a.at));
    Ok(out)
}

/// The facts the status rules judge a move on.
pub async fn facts(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket: &entity::maintenance_ticket::Model,
    scheduled_for: Option<String>,
    note: Option<String>,
    open_tasks_reason: Option<String>,
) -> ApiResult<Facts> {
    let tasks = TicketTask::find()
        .filter(entity::ticket_task::Column::TenantId.eq(tenant_id))
        .filter(entity::ticket_task::Column::TicketId.eq(ticket.id))
        .all(db)
        .await?;
    let counted: Vec<_> = tasks.iter().filter(|t| t.status != "skipped").collect();
    // A vendor who says they're finished has finished the task.
    let done = counted
        .iter()
        .filter(|t| t.status == "done" || t.vendor_response.as_deref() == Some("done"))
        .count() as i64;
    let now = Utc::now();
    let entries = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::MaintenanceTicketId.eq(ticket.id))
        .all(db)
        .await?;
    let minutes: i64 = entries.iter().map(|e| entry_minutes(e, now)).sum();
    let spent: i64 = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(tenant_id))
        .filter(entity::expense::Column::MaintenanceTicketId.eq(ticket.id))
        .all(db)
        .await?
        .iter()
        .map(|e| e.amount_cents)
        .sum();
    // A date set on the work order, or a booked visit, counts as scheduled.
    let booked = Appointment::find()
        .filter(entity::appointment::Column::TenantId.eq(tenant_id))
        .filter(entity::appointment::Column::SubjectType.eq("maintenance_ticket"))
        .filter(entity::appointment::Column::SubjectId.eq(ticket.id))
        .filter(entity::appointment::Column::Status.eq("scheduled"))
        .one(db)
        .await?
        .and_then(|a| a.starts_at.map(|s| s.date_naive().to_string()));
    Ok(Facts {
        open_tasks: counted.len() as i64 - done,
        total_tasks: counted.len() as i64,
        done_tasks: done,
        scheduled_for: scheduled_for
            .filter(|d| !d.trim().is_empty())
            .or_else(|| ticket.due_date.clone().filter(|d| !d.trim().is_empty()))
            .or(booked),
        note,
        open_tasks_reason,
        minutes,
        spent_cents: spent,
    })
}

/// Stop anyone clocked in on this work order. Returns who was stopped.
pub async fn stop_clocks(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    ticket_id: Uuid,
) -> ApiResult<Vec<String>> {
    let open = TimeEntry::find()
        .filter(entity::time_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::time_entry::Column::MaintenanceTicketId.eq(ticket_id))
        .filter(entity::time_entry::Column::EndedAt.is_null())
        .all(db)
        .await?;
    if open.is_empty() {
        return Ok(vec![]);
    }
    let rules = Rules::load(db, tenant_id).await;
    let mut stopped = vec![];
    for e in open {
        let uid = e.user_id;
        workforce::close_entry(db, tenant_id, e, Utc::now(), None, &rules).await?;
        if let Some(u) = User::find_by_id(uid).one(db).await? {
            stopped.push(u.name);
        }
    }
    Ok(stopped)
}
