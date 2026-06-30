//! `POST /leases/<id>/document/sign` — capture a typed signature on the latest
//! lease document. Signing **activates the tenancy** (lease → `active`) and syncs
//! the property's occupancy + unit status.

use super::dto::{LeaseDocDto, SignReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::guards::ClientIp;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Lease, LeaseDocument};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// SHA-256 (hex) of the document body — the tamper-evidence anchor.
fn body_hash(body: &str) -> String {
    let mut h = Sha256::new();
    h.update(body.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// `POST /leases/<id>/document/sign` — sign the latest document + activate the lease.
#[rocket_okapi::openapi(tag = "Lease Documents")]
#[post("/leases/<id>/document/sign", data = "<body>")]
pub async fn sign(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    client_ip: ClientIp,
    id: &str,
    body: Json<SignReq>,
) -> ApiResult<Json<LeaseDocDto>> {
    user.require(Permission::LeaseManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let b = body.into_inner();
    if b.signed_by.trim().is_empty() {
        return Err(ApiError::BadRequest("signed_by is required".into()));
    }
    let lease = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let doc = LeaseDocument::find()
        .filter(entity::lease_document::Column::LeaseId.eq(lid))
        .filter(entity::lease_document::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::lease_document::Column::GeneratedAt)
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("generate a document before signing".into()))?;
    if doc.status == "signed" {
        return Err(ApiError::Conflict("document is already signed".into()));
    }

    let now = Utc::now();
    let hash = body_hash(&doc.body);
    let mut dm: entity::lease_document::ActiveModel = doc.into();
    dm.status = Set("signed".into());
    dm.signed_at = Set(Some(now.into()));
    dm.signed_by = Set(Some(b.signed_by.clone()));
    dm.signed_hash = Set(Some(hash));
    dm.signed_ip = Set(client_ip.0.clone());
    let saved = dm.update(&state.db).await?;

    // Signing activates the tenancy.
    let property_id = lease.property_id;
    if lease.status != "active" {
        let mut lm: entity::lease::ActiveModel = lease.into();
        lm.status = Set("active".into());
        lm.updated_at = Set(now.into());
        lm.update(&state.db).await?;
    }
    crate::rentals_occupancy::sync_property_occupancy(&state.db, property_id).await;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::LEASE_DOC_SIGN,
        Some("lease_document"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": lid, "signed_by": b.signed_by })),
    )
    .await;
    Ok(Json(LeaseDocDto::from(saved)))
}
