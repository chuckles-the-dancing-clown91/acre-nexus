//! `POST /esign/envelopes/<id>/void` — cancel an outstanding envelope. Signing
//! links stop working, pending signers are told the request was cancelled, and
//! the lease document returns to `draft` so it can be revised and re-sent.

use super::dto::{EnvelopeDto, VoidReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::esign;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{EsignEnvelope, LeaseDocument};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /esign/envelopes/<id>/void` — cancel an outstanding envelope.
#[rocket_okapi::openapi(tag = "E-Signature")]
#[post("/esign/envelopes/<id>/void", data = "<body>")]
pub async fn void(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<VoidReq>,
) -> ApiResult<Json<EnvelopeDto>> {
    user.require(Permission::LeaseManage)?;
    let eid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let envelope = EsignEnvelope::find_by_id(eid)
        .filter(entity::esign_envelope::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("envelope not found".into()))?;
    if !super::is_open(&envelope.status) {
        return Err(ApiError::Conflict(format!(
            "envelope is already {}",
            envelope.status
        )));
    }
    let reason = body.into_inner().reason.filter(|r| !r.trim().is_empty());

    let now = Utc::now();
    let title = envelope.title.clone();
    let doc_id = envelope.lease_document_id;
    let mut em: entity::esign_envelope::ActiveModel = envelope.into();
    em.status = Set("voided".into());
    em.voided_at = Set(Some(now.into()));
    em.void_reason = Set(reason.clone());
    em.updated_at = Set(now.into());
    let envelope = em.update(&db).await?;

    // The document goes back to draft so it can be revised and re-sent.
    if let Some(doc) = LeaseDocument::find_by_id(doc_id)
        .filter(entity::lease_document::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
    {
        if doc.status == "sent" {
            let mut dm: entity::lease_document::ActiveModel = doc.into();
            dm.status = Set("draft".into());
            dm.update(&db).await?;
        }
    }

    esign::record_event(
        &db,
        scope.tenant_id,
        eid,
        None,
        "voided",
        json!({ "reason": reason }),
        None,
        None,
    )
    .await;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ESIGN_VOID,
        Some("esign_envelope"),
        Some(eid.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "reason": reason })),
    )
    .await;

    // Tell everyone who hadn't signed that no action is needed anymore.
    let signers = esign::envelope_signers(&db, scope.tenant_id, eid).await?;
    for s in signers.iter().filter(|s| s.status != "signed") {
        esign::notify_signer(
            &db,
            scope.tenant_id,
            s,
            "esign_voided",
            "voided",
            json!({ "document_title": title, "signer": s.name }),
        )
        .await;
    }

    let events = super::envelope_events(&db, scope.tenant_id, eid).await?;
    Ok(Json(EnvelopeDto::build(envelope, signers, events)))
}
