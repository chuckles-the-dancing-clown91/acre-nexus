//! `/my/tickets` — the **renter portal's** maintenance surface (Phase 5).
//! No staff permission required: everything is scoped to the signed-in
//! resident's own lease (matched by account email, like `/my/lease`).
//! Residents open requests, follow the timeline, add comments, and attach
//! photos through the document service.

use super::dto::{AddCommentReq, ReviewReq, TicketCommentDto, TicketDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::routes::documents::dto::{DocumentDto, UploadDocumentResp};
use crate::routes::documents::max_size_for;
use crate::state::AppState;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use schemars::JsonSchema;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::Deserialize;
use uuid::Uuid;

use entity::prelude::{Document, MaintenanceTicket, TicketComment};

const CATEGORIES: &[&str] = &[
    "plumbing",
    "electrical",
    "hvac",
    "appliance",
    "structural",
    "general",
];
const PRIORITIES: &[&str] = &["low", "normal", "high", "urgent"];

#[derive(Deserialize, JsonSchema)]
pub struct CreateMyTicketReq {
    pub title: String,
    pub description: Option<String>,
    /// `plumbing` | `electrical` | `hvac` | `appliance` | `structural` |
    /// `general` (default).
    pub category: Option<String>,
    /// `low` | `normal` (default) | `high` | `urgent`.
    pub priority: Option<String>,
    /// Where in the home (e.g. "Kitchen", "Master bathroom").
    pub location: Option<String>,
    /// Entry instructions ("lockbox on rail", "dog in yard").
    pub access_notes: Option<String>,
    /// The resident authorizes entry when they're not home.
    pub permission_to_enter: Option<bool>,
}

/// Register a photo (or other attachment) against a request.
#[derive(Deserialize, JsonSchema)]
pub struct MyTicketPhotoReq {
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: Option<i64>,
}

/// A ticket plus the resident-visible timeline and attachments.
#[derive(serde::Serialize, JsonSchema)]
pub struct MyTicketDetailResp {
    #[serde(flatten)]
    pub ticket: TicketDto,
    /// Comments + logged status changes, newest first.
    pub comments: Vec<TicketCommentDto>,
    /// Photos/attachments on the request, newest first.
    pub documents: Vec<DocumentDto>,
    /// The same files with a link to view them (photos and videos inline).
    pub files: Vec<MyFile>,
}

/// A photo, video or file the resident can see on their request.
#[derive(serde::Serialize, JsonSchema)]
pub struct MyFile {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    /// `photo` | `video` | `document`.
    pub kind: String,
    /// A signed link, good for 15 minutes.
    pub url: Option<String>,
    /// They uploaded it (rather than staff).
    pub mine: bool,
    pub created_at: String,
}

/// The signed-in resident's lease, or 404.
async fn my_lease(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<entity::lease::Model> {
    crate::payments::lease_for_user(db, tenant_id, user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("no lease found for your account".into()))
}

/// One of the resident's own tickets, or 404.
async fn my_ticket(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    lease_id: Uuid,
    id: &str,
) -> ApiResult<entity::maintenance_ticket::Model> {
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    MaintenanceTicket::find_by_id(tid)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
        .filter(entity::maintenance_ticket::Column::LeaseId.eq(lease_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("request not found".into()))
}

/// `GET /my/tickets` — the resident's maintenance requests, newest first.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/tickets")]
pub async fn my_tickets(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<TicketDto>>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let rows = MaintenanceTicket::find()
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_ticket::Column::LeaseId.eq(lease.id))
        .order_by_desc(entity::maintenance_ticket::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(TicketDto::from).collect()))
}

/// `POST /my/tickets` — open a maintenance request on the resident's own
/// lease. Lands on the staff maintenance board like any other work order.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/tickets", data = "<body>")]
pub async fn create_my_ticket(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateMyTicketReq>,
) -> ApiResult<Json<TicketDto>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let b = body.into_inner();
    let title = b.title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::BadRequest("title is required".into()));
    }
    let category = match b.category.as_deref().map(str::trim) {
        None | Some("") => "general".to_string(),
        Some(c) if CATEGORIES.contains(&c) => c.to_string(),
        Some(c) => {
            return Err(ApiError::BadRequest(format!(
                "invalid category: {c} (expected one of {})",
                CATEGORIES.join(", ")
            )))
        }
    };
    let priority = match b.priority.as_deref().map(str::trim) {
        None | Some("") => "normal".to_string(),
        Some(p) if PRIORITIES.contains(&p) => p.to_string(),
        Some(p) => {
            return Err(ApiError::BadRequest(format!(
                "invalid priority: {p} (expected one of {})",
                PRIORITIES.join(", ")
            )))
        }
    };

    let now = Utc::now();
    let (response_due, resolve_due) =
        crate::helpdesk::sla_targets(&db, scope.tenant_id, &priority, now).await;
    let saved = entity::maintenance_ticket::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(lease.property_id),
        unit_id: Set(lease.unit_id),
        lease_id: Set(Some(lease.id)),
        title: Set(title),
        description: Set(b.description.filter(|d| !d.trim().is_empty())),
        category: Set(category),
        priority: Set(priority),
        status: Set("open".to_string()),
        assignee_user_id: Set(None),
        assignee_entity_id: Set(None),
        reporter: Set(Some(lease.tenant_name.clone())),
        location: Set(b.location.filter(|s| !s.trim().is_empty())),
        access_notes: Set(b.access_notes.filter(|s| !s.trim().is_empty())),
        permission_to_enter: Set(b.permission_to_enter.unwrap_or(false)),
        asset_id: Set(None),
        waiting_on: Set(None),
        follow_up_date: Set(None),
        rating: Set(None),
        review_comment: Set(None),
        reviewed_at: Set(None),
        due_date: Set(None),
        cost_cents: Set(None),
        first_response_at: Set(None),
        resolved_at: Set(None),
        sla_response_due_at: Set(response_due.map(Into::into)),
        sla_resolve_due_at: Set(resolve_due.map(Into::into)),
        partner_counterparty_id: Set(None),
        partner_job_id: Set(None),
        partner_status: Set(None),
        partner_synced_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        track_time: Set(true),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_CREATE,
        Some("maintenance_ticket"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "property_id": saved.property_id,
            "lease_id": lease.id,
            "category": saved.category,
            "priority": saved.priority,
            "portal": true,
        })),
    )
    .await;

    // Outbound webhooks (#68): subscribed vendors hear about new work orders.
    crate::webhooks_out::emit(
        &db,
        scope.tenant_id,
        "maintenance_ticket.created",
        serde_json::json!({
            "ticket_id": saved.id,
            "property_id": saved.property_id,
            "category": saved.category,
            "priority": saved.priority,
            "status": saved.status,
        }),
    )
    .await;

    let property_address = entity::prelude::Property::find_by_id(saved.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .map(|p| p.address)
        .unwrap_or_default();
    crate::notify::notify_staff(
        &db,
        scope.tenant_id,
        "maintenance:read",
        "maintenance_request",
        serde_json::json!({
            "title": saved.title,
            "priority": saved.priority,
            "resident": lease.tenant_name,
            "property": property_address,
        }),
        Some(("maintenance_ticket", saved.id)),
        "created",
        Some(user.user_id),
    )
    .await;

    Ok(Json(TicketDto::from(saved)))
}

/// `GET /my/tickets/<id>` — one of the resident's requests with its timeline
/// (comments + status changes) and attachments.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/tickets/<id>")]
pub async fn my_ticket_detail(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<MyTicketDetailResp>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let ticket = my_ticket(&db, scope.tenant_id, lease.id, id).await?;

    // Residents see the public timeline only — internal notes stay staff-side.
    let comments = TicketComment::find()
        .filter(entity::ticket_comment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::ticket_comment::Column::TicketId.eq(ticket.id))
        .filter(entity::ticket_comment::Column::Kind.is_in(["comment", "status", "action"]))
        .filter(entity::ticket_comment::Column::Visibility.eq("public"))
        .order_by_desc(entity::ticket_comment::Column::CreatedAt)
        .all(&db)
        .await?;
    // Their own uploads, and files staff put on a public note; never staff
    // receipts or internal photos.
    let shared: std::collections::HashSet<Uuid> = comments
        .iter()
        .flat_map(|c| {
            serde_json::from_value::<Vec<Uuid>>(c.document_ids.clone()).unwrap_or_default()
        })
        .collect();
    let store = ObjectStore::from_env().ok();
    let mut q = Document::find()
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
        .filter(entity::document::Column::OwnerId.eq(ticket.id));
    // Locally, a file counts once its bytes land (S3 uploads go straight there).
    if matches!(store, Some(ObjectStore::Local(_))) {
        q = q.filter(entity::document::Column::Status.eq("stored"));
    }
    let documents: Vec<entity::document::Model> = q
        .order_by_desc(entity::document::Column::CreatedAt)
        .all(&db)
        .await?
        .into_iter()
        .filter(|d| d.created_by == Some(user.user_id) || shared.contains(&d.id))
        .collect();
    let files = documents
        .iter()
        .map(|d| MyFile {
            id: d.id,
            filename: d.filename.clone(),
            mime_type: d.mime_type.clone(),
            kind: if d.mime_type.starts_with("video/") {
                "video"
            } else if d.mime_type.starts_with("image/") {
                "photo"
            } else {
                "document"
            }
            .into(),
            url: store
                .as_ref()
                .and_then(|s| s.signed_get_url(&d.storage_key, SIGNED_URL_TTL_SECS).ok())
                .map(|s| s.url),
            mine: d.created_by == Some(user.user_id),
            created_at: d.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(MyTicketDetailResp {
        ticket: TicketDto::from(ticket),
        comments: comments.into_iter().map(TicketCommentDto::from).collect(),
        documents: documents.into_iter().map(DocumentDto::from).collect(),
        files,
    }))
}

/// `POST /my/tickets/<id>/comments` — add a comment to the resident's own
/// request.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/tickets/<id>/comments", data = "<body>")]
pub async fn add_my_comment(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<AddCommentReq>,
) -> ApiResult<Json<TicketCommentDto>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let ticket = my_ticket(&db, scope.tenant_id, lease.id, id).await?;
    let b = body.into_inner();
    let text = b.body.trim().to_string();
    if text.is_empty() && b.document_ids.is_empty() {
        return Err(ApiError::BadRequest(
            "write a comment or add a photo".into(),
        ));
    }
    // Attachments are their own uploads to this request.
    if !b.document_ids.is_empty() {
        let found = Document::find()
            .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
            .filter(entity::document::Column::OwnerId.eq(ticket.id))
            .filter(entity::document::Column::CreatedBy.eq(user.user_id))
            .filter(entity::document::Column::Id.is_in(b.document_ids.clone()))
            .all(&db)
            .await?;
        if found.len() != b.document_ids.len() {
            return Err(ApiError::BadRequest(
                "attach photos or videos you uploaded to this request".into(),
            ));
        }
    }

    let saved = entity::ticket_comment::ActiveModel {
        action: Set(Some("resident_comment".into())),
        document_ids: Set(serde_json::json!(b.document_ids)),
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        ticket_id: Set(ticket.id),
        task_id: Set(None),
        author_user_id: Set(Some(user.user_id)),
        kind: Set("comment".to_string()),
        visibility: Set("public".into()),
        author_name: Set(Some(lease.tenant_name.clone())),
        body: Set(text),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_COMMENT_ADD,
        Some("maintenance_ticket"),
        Some(ticket.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "portal": true })),
    )
    .await;

    crate::notify::notify_staff(
        &db,
        scope.tenant_id,
        "maintenance:read",
        "maintenance_request",
        serde_json::json!({
            "title": format!("{} (new comment)", ticket.title),
            "priority": ticket.priority,
            "resident": lease.tenant_name,
            "property": "",
        }),
        Some(("maintenance_ticket", ticket.id)),
        &format!("comment:{}", saved.id),
        Some(user.user_id),
    )
    .await;

    Ok(Json(TicketCommentDto::from(saved)))
}

/// `POST /my/tickets/<id>/photos` — register a photo against the resident's
/// own request and receive a short-lived signed `PUT` URL for the bytes
/// (the same two-step flow as the staff document service).
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/tickets/<id>/photos", data = "<body>")]
pub async fn add_my_ticket_photo(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<MyTicketPhotoReq>,
) -> ApiResult<Json<UploadDocumentResp>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let ticket = my_ticket(&db, scope.tenant_id, lease.id, id).await?;
    let b = body.into_inner();

    let filename = b.filename.trim().to_string();
    if filename.is_empty() || filename.contains('/') || filename.contains('\\') {
        return Err(ApiError::BadRequest("invalid filename".into()));
    }
    let mime_type = b.mime_type.trim().to_lowercase();
    if !(mime_type.starts_with("image/") || mime_type.starts_with("video/")) {
        return Err(ApiError::BadRequest("add a photo or a video".into()));
    }
    let size = b.size_bytes.unwrap_or(0);
    let max = max_size_for(&mime_type);
    if !(0..=max).contains(&size) {
        return Err(ApiError::BadRequest(format!(
            "that file is too large (up to {} MB)",
            max / 1024 / 1024
        )));
    }
    let category = if mime_type.starts_with("video/") {
        "video"
    } else {
        "photo"
    };

    let doc_id = Uuid::new_v4();
    let storage_key = format!("{}/{}", scope.tenant_id, doc_id);
    let now = Utc::now();
    let saved = entity::document::ActiveModel {
        id: Set(doc_id),
        tenant_id: Set(scope.tenant_id),
        owner_type: Set("maintenance_ticket".into()),
        owner_id: Set(ticket.id),
        filename: Set(filename.clone()),
        category: Set(Some(category.into())),
        requires_wet_ink: Set(false),
        physical_location: Set(None),
        mime_type: Set(mime_type),
        size_bytes: Set(size),
        checksum: Set(None),
        version: Set(1),
        previous_version_id: Set(None),
        storage_key: Set(storage_key.clone()),
        status: Set("pending_upload".into()),
        retention_expires_at: Set(None),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    let store = ObjectStore::from_env()?;
    let signed = store.signed_put_url(&storage_key, SIGNED_URL_TTL_SECS)?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DOCUMENT_UPLOAD,
        Some("document"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "owner_type": "maintenance_ticket",
            "owner_id": ticket.id,
            "filename": filename,
            "portal": true,
        })),
    )
    .await;

    Ok(Json(UploadDocumentResp {
        document: DocumentDto::from(saved),
        upload_url: signed.url,
        upload_url_expires_at: signed.expires_at.to_rfc3339(),
    }))
}

/// `POST /my/tickets/<id>/review` — the resident rates the completed repair
/// (1–5 + optional comment), once. Staff hear about it.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/tickets/<id>/review", data = "<body>")]
pub async fn review_my_ticket(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<ReviewReq>,
) -> ApiResult<Json<TicketDto>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let ticket = my_ticket(&db, scope.tenant_id, lease.id, id).await?;
    let b = body.into_inner();
    if !(1..=5).contains(&b.rating) {
        return Err(ApiError::BadRequest("rating must be 1–5".into()));
    }
    if !matches!(ticket.status.as_str(), "resolved" | "closed") {
        return Err(ApiError::BadRequest(
            "you can rate a repair once it's resolved".into(),
        ));
    }
    if ticket.rating.is_some() {
        return Err(ApiError::BadRequest(
            "this request has already been rated".into(),
        ));
    }

    let now = Utc::now();
    let rating = b.rating;
    let comment = b
        .comment
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty());
    let title = ticket.title.clone();
    let ticket_id = ticket.id;
    // Conditional on rating still being NULL: a double-submitted review
    // must not overwrite the first (the check above read pre-transaction
    // state).
    let updated = MaintenanceTicket::update_many()
        .col_expr(
            entity::maintenance_ticket::Column::Rating,
            Expr::value(rating),
        )
        .col_expr(
            entity::maintenance_ticket::Column::ReviewComment,
            Expr::value(comment.clone()),
        )
        .col_expr(
            entity::maintenance_ticket::Column::ReviewedAt,
            Expr::value(now),
        )
        .col_expr(
            entity::maintenance_ticket::Column::UpdatedAt,
            Expr::value(now),
        )
        .filter(entity::maintenance_ticket::Column::Id.eq(ticket_id))
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::maintenance_ticket::Column::Rating.is_null())
        .exec(&db)
        .await?;
    if updated.rows_affected == 0 {
        return Err(ApiError::BadRequest(
            "this request has already been rated".into(),
        ));
    }
    let saved = MaintenanceTicket::find_by_id(ticket_id)
        .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ticket not found".into()))?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TICKET_REVIEW,
        Some("maintenance_ticket"),
        Some(ticket_id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "rating": rating })),
    )
    .await;

    crate::notify::notify_staff(
        &db,
        scope.tenant_id,
        "maintenance:read",
        "ticket_reviewed",
        serde_json::json!({
            "title": title,
            "resident": lease.tenant_name,
            "rating": rating,
            "stars": "★".repeat(rating as usize),
            "comment": comment.unwrap_or_else(|| "(no comment)".into()),
        }),
        Some(("maintenance_ticket", ticket_id)),
        "reviewed",
        Some(user.user_id),
    )
    .await;

    Ok(Json(TicketDto::from(saved)))
}
