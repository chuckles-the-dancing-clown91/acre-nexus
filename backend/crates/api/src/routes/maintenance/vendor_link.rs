//! The **vendor's link**: every batch of tasks sent to a vendor carries one
//! link (`/vendor/<token>`). From it the vendor accepts or declines, says
//! when they can come, marks the work done, and sends photos and their
//! invoice. No account, no sign-in: the token is the credential, so these
//! routes only ever touch the tasks it was minted for.

use crate::error::{ApiError, ApiResult};
use crate::routes::appointments::{window_from, WindowReq};
use crate::routes::maintenance::desk::FileDto;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use chrono::Utc;
use entity::prelude::{
    Appointment, Counterparty, Document, Expense, MaintenanceTicket, Property, Tenant, TicketTask,
};
use rocket::serde::json::Json;
use rocket::{get, post};
use schemars::JsonSchema;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// Where a vendor link points.
pub fn url(token: &str) -> String {
    format!("{}/vendor/{token}", crate::oauth::public_app_url())
}

/// `$385.00`: invoices keep their cents.
fn money(cents: i64) -> String {
    format!(
        "{}.{:02}",
        crate::dto::usd(cents - cents.rem_euclid(100)),
        cents.rem_euclid(100)
    )
}

#[derive(Serialize, JsonSchema)]
pub struct VendorTaskDto {
    pub id: Uuid,
    pub title: String,
    pub trade: String,
    pub est_minutes: Option<i32>,
    pub status: String,
}

#[derive(Serialize, JsonSchema)]
pub struct VendorInvoiceDto {
    pub id: Uuid,
    pub description: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub incurred_on: String,
}

#[derive(Serialize, JsonSchema)]
pub struct VendorJob {
    /// The property manager's company.
    pub company: String,
    pub vendor: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub property: String,
    pub access_notes: Option<String>,
    pub due_date: Option<String>,
    pub note: Option<String>,
    pub tasks: Vec<VendorTaskDto>,
    /// `accepted` | `declined` | `done`, or null before they answer.
    pub response: Option<String>,
    pub responded_at: Option<String>,
    pub response_note: Option<String>,
    /// The visit they said they'd make, in words.
    pub when_words: Option<String>,
    pub files: Vec<FileDto>,
    pub invoices: Vec<VendorInvoiceDto>,
    pub timezone: String,
}

/// The tasks a link was minted for, and the work order they're on.
/// How a batch is named: the vendor's link token, or (for a signed-in vendor)
/// the stored hash of it, which the portal lists.
pub enum BatchKey<'a> {
    Token(&'a str),
    Hash(&'a str),
}

async fn by_batch(
    db: &impl ConnectionTrait,
    key: &BatchKey<'_>,
) -> ApiResult<(
    Vec<entity::ticket_task::Model>,
    entity::maintenance_ticket::Model,
    entity::counterparty::Model,
)> {
    let hash = match key {
        BatchKey::Token(token) => {
            let token = token.trim();
            if token.len() < 16 {
                return Err(ApiError::NotFound("that link isn't valid".into()));
            }
            crate::auth::hash_secret(token)
        }
        BatchKey::Hash(h) => h.to_string(),
    };
    let tasks = TicketTask::find()
        .filter(entity::ticket_task::Column::VendorTokenHash.eq(hash))
        .order_by_asc(entity::ticket_task::Column::Position)
        .all(db)
        .await?;
    let Some(first) = tasks.first() else {
        return Err(ApiError::NotFound("that link isn't valid".into()));
    };
    let ticket = MaintenanceTicket::find_by_id(first.ticket_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("that link isn't valid".into()))?;
    // Declining clears the assignee, so remember the vendor through the
    // dispatch note instead: any task in the batch still naming them wins.
    let vendor_id = tasks.iter().find_map(|t| t.assignee_entity_id).or_else(|| {
        first
            .vendor_note
            .as_deref()
            .and_then(|n| n.strip_prefix("vendor:"))
            .and_then(|v| v.split('\n').next())
            .and_then(|v| v.parse().ok())
    });
    let vendor = match vendor_id {
        Some(id) => Counterparty::find_by_id(id).one(db).await?,
        None => None,
    }
    .ok_or_else(|| ApiError::NotFound("that link isn't valid".into()))?;
    Ok((tasks, ticket, vendor))
}

async fn job_dto(
    db: &crate::db::RequestDb,
    tasks: Vec<entity::ticket_task::Model>,
    t: entity::maintenance_ticket::Model,
    vendor: entity::counterparty::Model,
) -> ApiResult<VendorJob> {
    let tz = crate::appointments::tz_for(db, t.tenant_id).await;
    let company = entity::prelude::Theme::find()
        .filter(entity::theme::Column::TenantId.eq(t.tenant_id))
        .one(db)
        .await?
        .map(|th| th.company_name);
    let company = match company {
        Some(c) => Some(c),
        None => Tenant::find_by_id(t.tenant_id)
            .one(db)
            .await?
            .map(|x| x.name),
    };
    let property = Property::find_by_id(t.property_id)
        .one(db)
        .await?
        .map(|p| format!("{}, {} {}", p.address, p.city, p.state))
        .unwrap_or_default();
    let first = tasks.first();
    let appointment = Appointment::find()
        .filter(entity::appointment::Column::TenantId.eq(t.tenant_id))
        .filter(entity::appointment::Column::SubjectType.eq("ticket"))
        .filter(entity::appointment::Column::SubjectId.eq(t.id))
        .filter(entity::appointment::Column::VendorEntityId.eq(vendor.id))
        .filter(entity::appointment::Column::Status.eq("confirmed"))
        .order_by_desc(entity::appointment::Column::CreatedAt)
        .one(db)
        .await?;
    let when_words = appointment.and_then(|a| match (a.starts_at, a.ends_at) {
        (Some(s), Some(e)) => Some(crate::appointments::window_words(
            &crate::appointments::Window {
                start: s.to_utc(),
                end: e.to_utc(),
            },
            &tz,
        )),
        _ => None,
    });
    let store = ObjectStore::from_env().ok();
    let files = Document::find()
        .filter(entity::document::Column::TenantId.eq(t.tenant_id))
        .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
        .filter(entity::document::Column::OwnerId.eq(t.id))
        .filter(entity::document::Column::CreatedBy.is_null())
        .order_by_desc(entity::document::Column::CreatedAt)
        .all(db)
        .await?
        .into_iter()
        .filter(|d| !matches!(store, Some(ObjectStore::Local(_))) || d.status == "stored")
        .map(|d| FileDto {
            url: store
                .as_ref()
                .and_then(|s| s.signed_get_url(&d.storage_key, SIGNED_URL_TTL_SECS).ok())
                .map(|s| s.url),
            kind: match d.category.as_deref() {
                Some("receipt") => "receipt",
                Some("video") => "video",
                Some("photo") => "photo",
                _ => "document",
            }
            .into(),
            id: d.id,
            filename: d.filename,
            mime_type: d.mime_type,
            size_bytes: d.size_bytes,
            created_at: d.created_at.to_rfc3339(),
        })
        .collect();
    let invoices = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(t.tenant_id))
        .filter(entity::expense::Column::MaintenanceTicketId.eq(t.id))
        .filter(entity::expense::Column::Vendor.eq(vendor.name.clone()))
        .filter(entity::expense::Column::RecordedBy.is_null())
        .order_by_desc(entity::expense::Column::CreatedAt)
        .all(db)
        .await?
        .into_iter()
        .map(|e| VendorInvoiceDto {
            id: e.id,
            description: e.description,
            amount_label: money(e.amount_cents),
            amount_cents: e.amount_cents,
            incurred_on: e.incurred_on,
        })
        .collect();
    Ok(VendorJob {
        company: company.unwrap_or_default(),
        vendor: vendor.name,
        title: t.title,
        description: t.description,
        priority: t.priority,
        property,
        access_notes: t.access_notes,
        due_date: t.due_date,
        note: first.and_then(|x| x.dispatch_note.clone()),
        response: first.and_then(|x| x.vendor_response.clone()),
        responded_at: first.and_then(|x| x.vendor_responded_at.map(|d| d.to_rfc3339())),
        response_note: first.and_then(|x| {
            x.vendor_note
                .as_deref()
                .and_then(|n| n.split_once('\n').map(|(_, rest)| rest.to_string()))
                .filter(|n| !n.is_empty())
        }),
        when_words,
        tasks: tasks
            .into_iter()
            .map(|x| VendorTaskDto {
                id: x.id,
                title: x.title,
                trade: x.trade,
                est_minutes: x.est_minutes,
                status: x.status,
            })
            .collect(),
        files,
        invoices,
        timezone: tz.name().to_string(),
    })
}

/// A line on the work order's timeline in the vendor's name.
async fn vendor_note(
    db: &impl ConnectionTrait,
    t: &entity::maintenance_ticket::Model,
    vendor: &str,
    action: &str,
    body: &str,
    visibility: &str,
) {
    let c = entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(t.tenant_id),
        ticket_id: Set(t.id),
        author_user_id: Set(None),
        kind: Set("action".into()),
        visibility: Set(visibility.into()),
        author_name: Set(Some(vendor.to_string())),
        body: Set(body.to_string()),
        document_ids: Set(json!([])),
        action: Set(Some(action.into())),
        created_at: Set(Utc::now().into()),
    };
    if let Err(e) = c.insert(db).await {
        tracing::error!("vendor note: {e}");
    }
}

/// Stamp the vendor's answer on every task in the batch. The note keeps the
/// vendor's id on its first line so a declined batch still knows who it was.
async fn stamp(
    db: &impl ConnectionTrait,
    tasks: &[entity::ticket_task::Model],
    vendor: &entity::counterparty::Model,
    response: &str,
    note: Option<&str>,
    mutate: impl Fn(&mut entity::ticket_task::ActiveModel),
) -> ApiResult<()> {
    let now = Utc::now();
    let stored = format!("vendor:{}\n{}", vendor.id, note.unwrap_or_default());
    for t in tasks {
        let mut am: entity::ticket_task::ActiveModel = t.clone().into();
        am.vendor_response = Set(Some(response.into()));
        am.vendor_responded_at = Set(Some(now.into()));
        am.vendor_note = Set(Some(stored.clone()));
        am.updated_at = Set(now.into());
        mutate(&mut am);
        am.update(db).await?;
    }
    Ok(())
}

fn clean(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

async fn staff_vars(
    db: &impl ConnectionTrait,
    t: &entity::maintenance_ticket::Model,
    vendor: &str,
) -> serde_json::Value {
    let property = Property::find_by_id(t.property_id)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|p| format!("{}, {}", p.address, p.city))
        .unwrap_or_default();
    json!({
        "vendor": vendor,
        "title": t.title,
        "property": property,
        "when": "",
        "note": "",
        "reason": "",
        "invoice": "",
    })
}

/// `GET /public/vendor/<token>` — what the link opens. Read-only, so a link
/// scanner changes nothing.
#[rocket_okapi::openapi(tag = "Vendors (Public)")]
#[get("/public/vendor/<token>")]
pub async fn view(db: crate::db::RequestDb, token: &str) -> ApiResult<Json<VendorJob>> {
    view_for(&db, &BatchKey::Token(token)).await
}

/// Which vendor a batch belongs to (for the portal's ownership check).
pub(crate) async fn batch_vendor(db: &impl ConnectionTrait, key: &BatchKey<'_>) -> ApiResult<Uuid> {
    Ok(by_batch(db, key).await?.2.id)
}

pub(crate) async fn view_for(
    db: &crate::db::RequestDb,
    key: &BatchKey<'_>,
) -> ApiResult<Json<VendorJob>> {
    let (tasks, t, vendor) = by_batch(db, key).await?;
    Ok(Json(job_dto(db, tasks, t, vendor).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct AcceptReq {
    pub note: Option<String>,
    /// When they'll come, if they know: local `YYYY-MM-DDTHH:MM` or RFC 3339.
    pub start: Option<String>,
    pub end: Option<String>,
}

/// `POST /public/vendor/<token>/accept` — the vendor takes the job, and may
/// say when they're coming. That books the visit on the calendar and the
/// resident hears.
#[rocket_okapi::openapi(tag = "Vendors (Public)")]
#[post("/public/vendor/<token>/accept", data = "<body>")]
pub async fn accept(
    db: crate::db::RequestDb,
    token: &str,
    body: Json<AcceptReq>,
) -> ApiResult<Json<VendorJob>> {
    accept_for(&db, &BatchKey::Token(token), body.into_inner()).await
}

pub(crate) async fn accept_for(
    db: &crate::db::RequestDb,
    key: &BatchKey<'_>,
    body: AcceptReq,
) -> ApiResult<Json<VendorJob>> {
    let (tasks, t, vendor) = by_batch(db, key).await?;
    if tasks
        .iter()
        .all(|x| matches!(x.status.as_str(), "done" | "skipped"))
    {
        return Err(ApiError::Conflict("this work is already done".into()));
    }
    let b = body;
    let note = clean(b.note);
    let now = Utc::now();
    stamp(db, &tasks, &vendor, "accepted", note.as_deref(), |am| {
        am.assignee_entity_id = Set(Some(vendor.id));
        if matches!(am.status.as_ref().as_str(), "todo") {
            am.status = Set("doing".into());
        }
    })
    .await?;
    let mut vars = staff_vars(db, &t, &vendor.name).await;
    let mut line = format!("{} accepted the work.", vendor.name);
    if let Some(start) = b.start.as_deref().filter(|s| !s.trim().is_empty()) {
        let w = window_from(
            db,
            t.tenant_id,
            WindowReq {
                start: start.to_string(),
                end: b.end.clone(),
            },
        )
        .await?;
        if w.start < now || w.end <= w.start {
            return Err(ApiError::BadRequest(
                "pick a time that's still ahead".into(),
            ));
        }
        // Their time replaces any other open visit on this work order.
        let open = Appointment::find()
            .filter(entity::appointment::Column::TenantId.eq(t.tenant_id))
            .filter(entity::appointment::Column::SubjectType.eq("ticket"))
            .filter(entity::appointment::Column::SubjectId.eq(t.id))
            .filter(entity::appointment::Column::Status.is_in(["proposed", "confirmed"]))
            .all(db)
            .await?;
        for a in open {
            let mut am: entity::appointment::ActiveModel = a.into();
            am.status = Set("cancelled".into());
            am.updated_at = Set(now.into());
            am.update(db).await?;
        }
        let resident = crate::appointments::resident_for_ticket(db, t.tenant_id, &t).await;
        let a = entity::appointment::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(t.tenant_id),
            property_id: Set(t.property_id),
            unit_id: Set(t.unit_id),
            kind: Set("repair".into()),
            subject_type: Set("ticket".into()),
            subject_id: Set(Some(t.id)),
            title: Set(t.title.clone()),
            status: Set("proposed".into()),
            windows: Set(json!([w])),
            starts_at: Set(None),
            ends_at: Set(None),
            with_name: Set(resident.as_ref().map(|l| l.tenant_name.clone())),
            with_email: Set(resident.as_ref().and_then(|l| l.tenant_email.clone())),
            with_phone: Set(resident.as_ref().and_then(|l| l.tenant_phone.clone())),
            with_role: Set("resident".into()),
            assignee_user_id: Set(None),
            vendor_entity_id: Set(Some(vendor.id)),
            note: Set(note.clone()),
            access_notes: Set(t.access_notes.clone()),
            token_hash: Set(None),
            confirmed_by: Set(None),
            confirmed_at: Set(None),
            proposed_start: Set(None),
            proposed_end: Set(None),
            reminded: Set(json!([])),
            outcome_note: Set(None),
            created_by: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(db)
        .await?;
        crate::appointments::confirm(db, t.tenant_id, a, w.clone(), "vendor").await?;
        let tz = crate::appointments::tz_for(db, t.tenant_id).await;
        let words = crate::appointments::window_words(&w, &tz);
        line = format!("{} accepted the work and is coming {words}.", vendor.name);
        vars["when"] = json!(format!(" They're coming {words}."));
    }
    if let Some(n) = &note {
        vars["note"] = json!(format!("\n\n\"{n}\""));
    }
    vendor_note(
        db,
        &t,
        &vendor.name,
        "vendor_accepted",
        &match &note {
            Some(n) => format!("{line} \"{n}\""),
            None => line,
        },
        "internal",
    )
    .await;
    crate::notify::notify_staff(
        db,
        t.tenant_id,
        "maintenance:manage",
        "vendor_task_accepted",
        vars,
        Some(("maintenance_ticket", t.id)),
        &format!("vendor_accepted:{}:{}", tasks[0].id, now.timestamp()),
        None,
    )
    .await;
    let (tasks, t, vendor) = by_batch(db, key).await?;
    Ok(Json(job_dto(db, tasks, t, vendor).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct DeclineReq {
    pub reason: Option<String>,
}

/// `POST /public/vendor/<token>/decline` — they can't take it. The tasks go
/// back to unassigned and staff hear, so it can go to someone else.
#[rocket_okapi::openapi(tag = "Vendors (Public)")]
#[post("/public/vendor/<token>/decline", data = "<body>")]
pub async fn decline(
    db: crate::db::RequestDb,
    token: &str,
    body: Json<DeclineReq>,
) -> ApiResult<Json<VendorJob>> {
    decline_for(&db, &BatchKey::Token(token), body.into_inner()).await
}

pub(crate) async fn decline_for(
    db: &crate::db::RequestDb,
    key: &BatchKey<'_>,
    body: DeclineReq,
) -> ApiResult<Json<VendorJob>> {
    let (tasks, t, vendor) = by_batch(db, key).await?;
    if tasks
        .iter()
        .any(|x| x.vendor_response.as_deref() == Some("done"))
    {
        return Err(ApiError::Conflict("this work is already done".into()));
    }
    let reason = clean(body.reason);
    let now = Utc::now();
    stamp(db, &tasks, &vendor, "declined", reason.as_deref(), |am| {
        am.assignee_entity_id = Set(None);
        am.dispatched_at = Set(None);
        am.dispatch_via = Set(None);
        if matches!(am.status.as_ref().as_str(), "doing") {
            am.status = Set("todo".into());
        }
    })
    .await?;
    vendor_note(
        db,
        &t,
        &vendor.name,
        "vendor_declined",
        &match &reason {
            Some(r) => format!("{} declined the work: \"{r}\"", vendor.name),
            None => format!("{} declined the work.", vendor.name),
        },
        "internal",
    )
    .await;
    let mut vars = staff_vars(db, &t, &vendor.name).await;
    if let Some(r) = &reason {
        vars["reason"] = json!(format!(" They said: \"{r}\""));
    }
    crate::notify::notify_staff(
        db,
        t.tenant_id,
        "maintenance:manage",
        "vendor_task_declined",
        vars,
        Some(("maintenance_ticket", t.id)),
        &format!("vendor_declined:{}:{}", tasks[0].id, now.timestamp()),
        None,
    )
    .await;
    let (tasks, t, vendor) = by_batch(db, key).await?;
    Ok(Json(job_dto(db, tasks, t, vendor).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct DoneReq {
    pub note: Option<String>,
}

/// `POST /public/vendor/<token>/done` — the work's finished. Their tasks
/// close, the resident sees the line, and staff review it.
#[rocket_okapi::openapi(tag = "Vendors (Public)")]
#[post("/public/vendor/<token>/done", data = "<body>")]
pub async fn done(
    db: crate::db::RequestDb,
    token: &str,
    body: Json<DoneReq>,
) -> ApiResult<Json<VendorJob>> {
    done_for(&db, &BatchKey::Token(token), body.into_inner()).await
}

pub(crate) async fn done_for(
    db: &crate::db::RequestDb,
    key: &BatchKey<'_>,
    body: DoneReq,
) -> ApiResult<Json<VendorJob>> {
    let (tasks, t, vendor) = by_batch(db, key).await?;
    if tasks
        .iter()
        .any(|x| x.vendor_response.as_deref() == Some("declined"))
    {
        return Err(ApiError::Conflict(
            "this work was declined; ask the office to send it again".into(),
        ));
    }
    let note = clean(body.note);
    let now = Utc::now();
    stamp(db, &tasks, &vendor, "done", note.as_deref(), |am| {
        am.assignee_entity_id = Set(Some(vendor.id));
        if !matches!(am.status.as_ref().as_str(), "done" | "skipped") {
            am.status = Set("done".into());
            am.done_at = Set(Some(now.into()));
        }
    })
    .await?;
    vendor_note(
        db,
        &t,
        &vendor.name,
        "vendor_done",
        &match &note {
            Some(n) => format!("{} finished their work. \"{n}\"", vendor.name),
            None => format!("{} finished their work.", vendor.name),
        },
        "public",
    )
    .await;
    let invoices = Expense::find()
        .filter(entity::expense::Column::TenantId.eq(t.tenant_id))
        .filter(entity::expense::Column::MaintenanceTicketId.eq(t.id))
        .filter(entity::expense::Column::Vendor.eq(vendor.name.clone()))
        .filter(entity::expense::Column::RecordedBy.is_null())
        .all(db)
        .await?;
    let mut vars = staff_vars(db, &t, &vendor.name).await;
    if let Some(n) = &note {
        vars["note"] = json!(format!(" \"{n}\""));
    }
    if !invoices.is_empty() {
        let total: i64 = invoices.iter().map(|e| e.amount_cents).sum();
        vars["invoice"] = json!(format!(" Their invoice: {}.", money(total)));
    }
    crate::notify::notify_staff(
        db,
        t.tenant_id,
        "maintenance:manage",
        "vendor_task_done",
        vars,
        Some(("maintenance_ticket", t.id)),
        &format!("vendor_done:{}:{}", tasks[0].id, now.timestamp()),
        None,
    )
    .await;
    let (tasks, t, vendor) = by_batch(db, key).await?;
    Ok(Json(job_dto(db, tasks, t, vendor).await?))
}

#[derive(Deserialize, JsonSchema)]
pub struct VendorUploadReq {
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
    /// `photo` (default for images, `video` for videos) | `invoice`.
    pub kind: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct VendorUploadResp {
    pub file: FileDto,
    /// `PUT` the bytes here.
    pub upload_url: String,
}

/// `POST /public/vendor/<token>/uploads` — a before/after photo or the
/// invoice as a file. Returns a signed URL to `PUT` the bytes to.
#[rocket_okapi::openapi(tag = "Vendors (Public)")]
#[post("/public/vendor/<token>/uploads", data = "<body>")]
pub async fn upload(
    db: crate::db::RequestDb,
    token: &str,
    body: Json<VendorUploadReq>,
) -> ApiResult<Json<VendorUploadResp>> {
    upload_for(&db, &BatchKey::Token(token), body.into_inner()).await
}

pub(crate) async fn upload_for(
    db: &crate::db::RequestDb,
    key: &BatchKey<'_>,
    body: VendorUploadReq,
) -> ApiResult<Json<VendorUploadResp>> {
    let (tasks, t, _vendor) = by_batch(db, key).await?;
    if tasks
        .iter()
        .any(|x| x.vendor_response.as_deref() == Some("declined"))
    {
        return Err(ApiError::Conflict("this work was declined".into()));
    }
    let b = body;
    let filename = b.filename.trim().to_string();
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return Err(ApiError::BadRequest("invalid filename".into()));
    }
    let mime = b.mime_type.trim().to_lowercase();
    if mime.is_empty() {
        return Err(ApiError::BadRequest("mime_type is required".into()));
    }
    let size = b.size_bytes.unwrap_or(0);
    if !(0..=crate::routes::documents::max_size_for(&mime)).contains(&size) {
        return Err(ApiError::BadRequest("that file is too large".into()));
    }
    let kind = match b.kind.as_deref().map(str::trim) {
        Some("invoice") | Some("receipt") => "receipt",
        None | Some("photo") | Some("video") if mime.starts_with("video/") => "video",
        None | Some("photo") if mime.starts_with("image/") => "photo",
        None | Some("photo") => {
            return Err(ApiError::BadRequest("a photo has to be an image".into()))
        }
        Some(k) => {
            return Err(ApiError::BadRequest(format!(
                "kind must be photo or invoice, not {k}"
            )))
        }
    };
    let doc_id = Uuid::new_v4();
    let key = format!("{}/{}", t.tenant_id, doc_id);
    let now = Utc::now();
    let saved = entity::document::ActiveModel {
        id: Set(doc_id),
        tenant_id: Set(t.tenant_id),
        owner_type: Set("maintenance_ticket".into()),
        owner_id: Set(t.id),
        filename: Set(filename),
        category: Set(Some(kind.into())),
        requires_wet_ink: Set(false),
        physical_location: Set(None),
        mime_type: Set(mime),
        size_bytes: Set(size),
        checksum: Set(None),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(key.clone()),
        status: Set("pending_upload".into()),
        retention_expires_at: Set(None),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    let signed = ObjectStore::from_env()?.signed_put_url(&key, SIGNED_URL_TTL_SECS)?;
    Ok(Json(VendorUploadResp {
        file: FileDto {
            id: saved.id,
            filename: saved.filename,
            mime_type: saved.mime_type,
            kind: kind.into(),
            size_bytes: saved.size_bytes,
            url: None,
            created_at: saved.created_at.to_rfc3339(),
        },
        upload_url: signed.url,
    }))
}

#[derive(Deserialize, JsonSchema)]
pub struct InvoiceReq {
    pub amount_cents: i64,
    pub description: Option<String>,
    /// The invoice file, if they uploaded one first.
    pub document_id: Option<Uuid>,
}

/// `POST /public/vendor/<token>/invoice` — what they're charging. Lands as a
/// vendor expense on the work order, billable to the owner, for the office
/// to approve and pay.
#[rocket_okapi::openapi(tag = "Vendors (Public)")]
#[post("/public/vendor/<token>/invoice", data = "<body>")]
pub async fn invoice(
    db: crate::db::RequestDb,
    token: &str,
    body: Json<InvoiceReq>,
) -> ApiResult<Json<VendorJob>> {
    invoice_for(&db, &BatchKey::Token(token), body.into_inner()).await
}

pub(crate) async fn invoice_for(
    db: &crate::db::RequestDb,
    key: &BatchKey<'_>,
    body: InvoiceReq,
) -> ApiResult<Json<VendorJob>> {
    let (tasks, t, vendor) = by_batch(db, key).await?;
    if tasks
        .iter()
        .any(|x| x.vendor_response.as_deref() == Some("declined"))
    {
        return Err(ApiError::Conflict("this work was declined".into()));
    }
    let b = body;
    if b.amount_cents <= 0 {
        return Err(ApiError::BadRequest(
            "the amount must be more than zero".into(),
        ));
    }
    if b.amount_cents > 1_000_000_000 {
        return Err(ApiError::BadRequest("that amount looks wrong".into()));
    }
    let receipts: Vec<Uuid> = match b.document_id {
        Some(id) => {
            Document::find_by_id(id)
                .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
                .filter(entity::document::Column::OwnerId.eq(t.id))
                .one(db)
                .await?
                .ok_or_else(|| {
                    ApiError::BadRequest("upload the invoice to this work order first".into())
                })?;
            vec![id]
        }
        None => vec![],
    };
    let description = clean(b.description).unwrap_or_else(|| {
        format!(
            "{} invoice: {}",
            vendor.name,
            tasks
                .iter()
                .map(|x| x.title.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )
    });
    let now = Utc::now();
    entity::expense::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(t.tenant_id),
        incurred_on: Set(now.date_naive().to_string()),
        category: Set("repairs".into()),
        vendor: Set(Some(vendor.name.clone())),
        description: Set(description),
        amount_cents: Set(b.amount_cents),
        miles_hundredths: Set(None),
        mileage_rate_mills: Set(None),
        tax_deductible: Set(true),
        vehicle: Set("none".into()),
        reimbursable: Set(false),
        reimbursed_at: Set(None),
        billable_to_owner: Set(true),
        billed_bill_id: Set(None),
        user_id: Set(None),
        maintenance_ticket_id: Set(Some(t.id)),
        rehab_project_id: Set(None),
        property_id: Set(Some(t.property_id)),
        asset_id: Set(t.asset_id),
        details: Set(json!({
            "receipt_document_ids": receipts,
            "vendor_entity_id": vendor.id,
            "task_ids": tasks.iter().map(|x| x.id).collect::<Vec<_>>(),
            "source": "vendor_link",
        })),
        recorded_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    vendor_note(
        db,
        &t,
        &vendor.name,
        "vendor_invoice",
        &format!(
            "{} sent an invoice for {}.",
            vendor.name,
            money(b.amount_cents)
        ),
        "internal",
    )
    .await;
    let (tasks, t, vendor) = by_batch(db, key).await?;
    Ok(Json(job_dto(db, tasks, t, vendor).await?))
}

#[cfg(test)]
mod tests {
    #[test]
    fn money_keeps_cents() {
        assert_eq!(super::money(38500), "$385.00");
        assert_eq!(super::money(123456), "$1,234.56");
        assert_eq!(super::money(5), "$0.05");
    }
}
