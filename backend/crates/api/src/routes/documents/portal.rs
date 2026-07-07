//! `/my/documents` — the **renter portal's** document surface (Phase 5).
//! No staff permission required: everything is scoped to documents owned by
//! the signed-in resident's own lease (the signed lease PDF, receipts,
//! statements) or by records hanging off it (their maintenance tickets'
//! photos, their inspections' photos).

use super::dto::{DocumentDto, DownloadDocumentResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::storage::{ObjectStore, SIGNED_URL_TTL_SECS};
use crate::tenancy::TenantScope;
use entity::prelude::{Document, Inspection, MaintenanceTicket};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// The signed-in resident's lease — current preferred, past accepted (a
/// moved-out resident still reads their documents) — or 404.
async fn my_lease(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<entity::lease::Model> {
    crate::payments::any_lease_for_user(db, tenant_id, user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("no lease found for your account".into()))
}

/// Whether a document hangs off the resident's own lease — directly (`lease`
/// owner) or through one of their maintenance tickets / inspections.
async fn document_is_mine(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    lease: &entity::lease::Model,
    doc: &entity::document::Model,
) -> ApiResult<bool> {
    Ok(match doc.owner_type.as_str() {
        "lease" => doc.owner_id == lease.id,
        "maintenance_ticket" => MaintenanceTicket::find_by_id(doc.owner_id)
            .filter(entity::maintenance_ticket::Column::TenantId.eq(tenant_id))
            .filter(entity::maintenance_ticket::Column::LeaseId.eq(lease.id))
            .one(db)
            .await?
            .is_some(),
        "inspection" => Inspection::find_by_id(doc.owner_id)
            .filter(entity::inspection::Column::TenantId.eq(tenant_id))
            .filter(entity::inspection::Column::LeaseId.eq(lease.id))
            .one(db)
            .await?
            .is_some(),
        _ => false,
    })
}

/// `GET /my/documents` — the resident's lease documents, newest first: the
/// signed lease, receipts, and statements filed against their tenancy.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/documents")]
pub async fn my_documents(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<DocumentDto>>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let rows = Document::find()
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::document::Column::OwnerType.eq("lease"))
        .filter(entity::document::Column::OwnerId.eq(lease.id))
        .order_by_desc(entity::document::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(DocumentDto::from).collect()))
}

/// `GET /my/documents/<id>/download` — signed download URL for a document
/// belonging to the resident's own lease (directly, or via their maintenance
/// tickets / inspections). Access is audited like the staff endpoint.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/documents/<id>/download")]
pub async fn my_document_download(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<DownloadDocumentResp>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    let doc = Document::find_by_id(id)
        .filter(entity::document::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("document not found".into()))?;
    if !document_is_mine(&db, scope.tenant_id, &lease, &doc).await? {
        return Err(ApiError::NotFound("document not found".into()));
    }

    let store = ObjectStore::from_env()?;
    let signed = store.signed_get_url(&doc.storage_key, SIGNED_URL_TTL_SECS)?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::DOCUMENT_DOWNLOAD,
        Some("document"),
        Some(doc.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "filename": doc.filename,
            "owner_type": doc.owner_type,
            "owner_id": doc.owner_id,
            "portal": true,
        })),
    )
    .await;

    Ok(Json(DownloadDocumentResp {
        url: signed.url,
        expires_at: signed.expires_at.to_rfc3339(),
    }))
}
