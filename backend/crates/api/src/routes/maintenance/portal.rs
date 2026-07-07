//! `/my/tickets` — the **renter portal's** maintenance surface (Phase 5).
//! No staff permission required: everything is scoped to the signed-in
//! resident's own lease (matched by account email, like `/my/lease`).
//! Residents open requests, follow the timeline, add comments, and attach
//! photos through the document service.

use super::dto::{AddCommentReq, TicketCommentDto, TicketDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::routes::documents::dto::{DocumentDto, UploadDocumentResp};
use crate::routes::documents::MAX_SIZE_BYTES;
use crate::state::AppState;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{get, post, State};
use schemars::JsonSchema;
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
        due_date: Set(None),
        cost_cents: Set(None),
        first_response_at: Set(None),
        resolved_at: Set(None),
        sla_response_due_at: Set(response_due.map(Into::into)),
        sla_resolve_due_at: Set(resolve_due.map(Into::into)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
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
        .filter(entity::ticket_comment::Column::Kind.is_in(["comment", "status"]))
        .filter(entity::ticket_comment::Column::Visibility.eq("public"))
        .order_by_desc(entity::ticket_comment::Column::CreatedAt)
        .all(&db)
        .await?;
    let documents = Document::find()
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::document::Column::OwnerType.eq("maintenance_ticket"))
        .filter(entity::document::Column::OwnerId.eq(ticket.id))
        .order_by_desc(entity::document::Column::CreatedAt)
        .all(&db)
        .await?;

    Ok(Json(MyTicketDetailResp {
        ticket: TicketDto::from(ticket),
        comments: comments.into_iter().map(TicketCommentDto::from).collect(),
        documents: documents.into_iter().map(DocumentDto::from).collect(),
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
    let text = body.into_inner().body.trim().to_string();
    if text.is_empty() {
        return Err(ApiError::BadRequest("comment body is required".into()));
    }

    let saved = entity::ticket_comment::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        ticket_id: Set(ticket.id),
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
    let mime_type = b.mime_type.trim().to_string();
    if mime_type.is_empty() {
        return Err(ApiError::BadRequest("mime_type is required".into()));
    }
    let size = b.size_bytes.unwrap_or(0);
    if !(0..=MAX_SIZE_BYTES).contains(&size) {
        return Err(ApiError::BadRequest(format!(
            "size_bytes must be between 0 and {MAX_SIZE_BYTES}"
        )));
    }

    let doc_id = Uuid::new_v4();
    let storage_key = format!("{}/{}", scope.tenant_id, doc_id);
    let now = Utc::now();
    let saved = entity::document::ActiveModel {
        id: Set(doc_id),
        tenant_id: Set(scope.tenant_id),
        owner_type: Set("maintenance_ticket".into()),
        owner_id: Set(ticket.id),
        filename: Set(filename.clone()),
        category: Set(Some("other".into())),
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
