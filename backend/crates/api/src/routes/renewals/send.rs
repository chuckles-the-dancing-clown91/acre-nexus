//! `POST /renewals/<id>/send` — send a proposed renewal's addendum out for
//! e-signature (resident + landlord by default), reusing the Phase 2 envelope
//! engine with `purpose = "renewal"` so completion applies the new terms.

use super::dto::{RenewalDto, SendRenewalReq, SendRenewalResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::esign;
use crate::rbac::Permission;
use crate::routes::esign::create::resolve_signers;
use crate::routes::esign::dto::{EnvelopeDto, SignerLink};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{Lease, LeaseDocument, LeaseRenewal};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /renewals/<id>/send` — send the addendum for signature.
#[rocket_okapi::openapi(tag = "Lease Renewals")]
#[post("/renewals/<id>/send", data = "<body>")]
pub async fn send(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<SendRenewalReq>,
) -> ApiResult<Json<SendRenewalResp>> {
    user.require(Permission::LeaseManage)?;
    let rid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let b = body.into_inner();

    let renewal = LeaseRenewal::find_by_id(rid)
        .filter(entity::lease_renewal::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("renewal not found".into()))?;
    if renewal.status != "proposed" {
        return Err(ApiError::Conflict(format!(
            "renewal is '{}' — only a proposed renewal can be sent",
            renewal.status
        )));
    }
    let doc_id = renewal
        .lease_document_id
        .ok_or_else(|| ApiError::Conflict("renewal has no addendum document".into()))?;

    let lease = Lease::find_by_id(renewal.lease_id)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let doc = LeaseDocument::find_by_id(doc_id)
        .filter(entity::lease_document::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("addendum document not found".into()))?;

    let signers = resolve_signers(&db, &lease, &user, b.signers).await?;
    let (envelope, saved_signers, raw_links) = esign::issue_envelope(
        &db,
        scope.tenant_id,
        user.user_id,
        &lease,
        &doc,
        "renewal",
        b.message,
        signers,
    )
    .await?;

    let now = Utc::now();
    let mut rm: entity::lease_renewal::ActiveModel = renewal.into();
    rm.status = Set("sent".into());
    rm.envelope_id = Set(Some(envelope.id));
    rm.updated_at = Set(now.into());
    let renewal = rm.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::LEASE_RENEWAL_SEND,
        Some("lease_renewal"),
        Some(renewal.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "lease_id": lease.id, "envelope_id": envelope.id })),
    )
    .await;

    let links = SignerLink::from_pairs(&saved_signers, &raw_links);
    let events = crate::routes::esign::envelope_events(&db, scope.tenant_id, envelope.id).await?;
    Ok(Json(SendRenewalResp {
        renewal: RenewalDto::from(renewal),
        envelope: EnvelopeDto::build(envelope, saved_signers, events),
        sign_links: links,
    }))
}
