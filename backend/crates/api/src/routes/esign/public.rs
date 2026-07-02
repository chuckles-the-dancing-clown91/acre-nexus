//! Public (unauthenticated) signer endpoints — what the emailed/texted signing
//! links hit. Possession of the raw token is the credential: we look the
//! signer up by the token's SHA-256, tenant-scoped via the same `X-Tenant` /
//! `?tenant=` resolution the public apply funnel uses. Every view/sign/decline
//! lands in the ESIGN/UETA audit trail with IP + user agent.

use super::dto::{DeclineReq, PublicCoSigner, PublicSignView, SignerDto, SubmitSignatureReq};
use crate::error::{ApiError, ApiResult};
use crate::esign;
use crate::guards::{ClientIp, UserAgent};
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use chrono::Utc;
use entity::prelude::{EsignEnvelope, EsignSigner, LeaseDocument, Theme};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;

/// Resolve a raw signing token to its (signer, envelope) pair.
async fn signer_for_token(
    db: &crate::db::RequestDb,
    tenant_id: uuid::Uuid,
    token: &str,
) -> ApiResult<(entity::esign_signer::Model, entity::esign_envelope::Model)> {
    let hash = esign::hash_token(token);
    let signer = EsignSigner::find()
        .filter(entity::esign_signer::Column::TenantId.eq(tenant_id))
        .filter(entity::esign_signer::Column::TokenHash.eq(hash))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("signing link is invalid or has expired".into()))?;
    let envelope = EsignEnvelope::find_by_id(signer.envelope_id)
        .filter(entity::esign_envelope::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("envelope not found".into()))?;
    Ok((signer, envelope))
}

/// Build the signer-scoped view of an envelope.
async fn build_view(
    db: &crate::db::RequestDb,
    tenant_id: uuid::Uuid,
    signer: entity::esign_signer::Model,
    envelope: &entity::esign_envelope::Model,
) -> ApiResult<PublicSignView> {
    let company = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .map(|t| t.company_name)
        .unwrap_or_else(|| "Acre Nexus".into());
    // Voided envelopes hide the document text — the link is dead.
    let document_body = if envelope.status == "voided" {
        None
    } else {
        LeaseDocument::find_by_id(envelope.lease_document_id)
            .filter(entity::lease_document::Column::TenantId.eq(tenant_id))
            .one(db)
            .await?
            .map(|d| d.body)
    };
    let co_signers = esign::envelope_signers(db, tenant_id, envelope.id)
        .await?
        .into_iter()
        .filter(|s| s.id != signer.id)
        .map(|s| PublicCoSigner {
            name: s.name,
            role: s.role,
            status: s.status,
        })
        .collect();
    Ok(PublicSignView {
        company,
        envelope_status: envelope.status.clone(),
        document_title: envelope.title.clone(),
        document_body,
        body_hash: envelope.body_hash.clone(),
        message: envelope.message.clone(),
        signer: SignerDto::from(signer),
        co_signers,
    })
}

/// `GET /public/sign/<token>` — the signer's view of the document. First open
/// marks the signer `viewed` (an ESIGN audit event with IP + user agent).
#[rocket_okapi::openapi(tag = "E-Signature (Public)")]
#[get("/public/sign/<token>")]
pub async fn view(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    client_ip: ClientIp,
    user_agent: UserAgent,
    token: &str,
) -> ApiResult<Json<PublicSignView>> {
    let (signer, envelope) = signer_for_token(&db, tenant.tenant_id, token).await?;

    // First open: sent → viewed.
    let signer = if signer.status == "sent" && super::is_open(&envelope.status) {
        let now = Utc::now();
        let mut am: entity::esign_signer::ActiveModel = signer.into();
        am.status = Set("viewed".into());
        am.viewed_at = Set(Some(now.into()));
        am.updated_at = Set(now.into());
        let saved = am.update(&db).await?;
        esign::record_event(
            &db,
            tenant.tenant_id,
            envelope.id,
            Some(saved.id),
            "viewed",
            json!({ "signer": saved.name }),
            client_ip.0.clone(),
            user_agent.0.clone(),
        )
        .await;
        crate::audit::record(
            &db,
            None,
            crate::audit::actions::ESIGN_VIEW,
            Some("esign_envelope"),
            Some(envelope.id.to_string()),
            Some(tenant.tenant_id),
            Some(json!({ "signer_id": saved.id })),
        )
        .await;
        saved
    } else {
        signer
    };

    Ok(Json(
        build_view(&db, tenant.tenant_id, signer, &envelope).await?,
    ))
}

/// `POST /public/sign/<token>` — capture the typed signature. When this was
/// the last outstanding signer the envelope completes: the lease document is
/// marked signed, the lease activates, the signed PDF is stored, and everyone
/// is notified.
#[rocket_okapi::openapi(tag = "E-Signature (Public)")]
#[post("/public/sign/<token>", data = "<body>")]
pub async fn sign(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    client_ip: ClientIp,
    user_agent: UserAgent,
    token: &str,
    body: Json<SubmitSignatureReq>,
) -> ApiResult<Json<PublicSignView>> {
    let b = body.into_inner();
    let signed_name = b.signed_name.trim().to_string();
    if signed_name.is_empty() {
        return Err(ApiError::BadRequest("type your full name to sign".into()));
    }
    if !b.consent {
        return Err(ApiError::BadRequest(
            "you must consent to sign electronically".into(),
        ));
    }

    let (signer, envelope) = signer_for_token(&db, tenant.tenant_id, token).await?;
    if !super::is_open(&envelope.status) {
        return Err(ApiError::Conflict(format!(
            "this envelope is {} and can no longer be signed",
            envelope.status
        )));
    }
    match signer.status.as_str() {
        "signed" => return Err(ApiError::Conflict("you have already signed".into())),
        "declined" => {
            return Err(ApiError::Conflict(
                "you declined this document — ask the sender for a new envelope".into(),
            ))
        }
        _ => {}
    }

    let now = Utc::now();
    // Signing from a link without a recorded view still counts as viewing.
    let viewed_at = signer.viewed_at.or(Some(now.into()));
    let mut am: entity::esign_signer::ActiveModel = signer.into();
    am.status = Set("signed".into());
    am.signed_at = Set(Some(now.into()));
    am.signed_name = Set(Some(signed_name.clone()));
    am.signed_ip = Set(client_ip.0.clone());
    am.signed_user_agent = Set(user_agent.0.clone());
    am.viewed_at = Set(viewed_at);
    am.updated_at = Set(now.into());
    let signer = am.update(&db).await?;

    esign::record_event(
        &db,
        tenant.tenant_id,
        envelope.id,
        Some(signer.id),
        "signed",
        json!({ "signer": signer.name, "signed_name": signed_name, "consent": true }),
        client_ip.0.clone(),
        user_agent.0.clone(),
    )
    .await;
    crate::audit::record(
        &db,
        None,
        crate::audit::actions::ESIGN_SIGN,
        Some("esign_envelope"),
        Some(envelope.id.to_string()),
        Some(tenant.tenant_id),
        Some(json!({ "signer_id": signer.id })),
    )
    .await;

    let signers = esign::envelope_signers(&db, tenant.tenant_id, envelope.id).await?;
    let envelope = if esign::all_signed(&signers) {
        esign::complete_envelope(&db, &envelope, &signers).await?;
        EsignEnvelope::find_by_id(envelope.id)
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("envelope not found".into()))?
    } else {
        // Someone still pending: mark progress + tell the back office.
        let signed_count = signers.iter().filter(|s| s.status == "signed").count();
        let mut em: entity::esign_envelope::ActiveModel = envelope.clone().into();
        em.status = Set("partially_signed".into());
        em.updated_at = Set(now.into());
        let updated = em.update(&db).await?;
        crate::notify::notify_staff(
            &db,
            tenant.tenant_id,
            "lease:read",
            "esign_signed_staff",
            json!({
                "signer": signer.name,
                "document_title": updated.title,
                "signed_count": signed_count.to_string(),
                "signer_count": signers.len().to_string(),
            }),
            Some(("esign_envelope", updated.id)),
            &format!("signed:{}", signer.id),
            None,
        )
        .await;
        updated
    };

    Ok(Json(
        build_view(&db, tenant.tenant_id, signer, &envelope).await?,
    ))
}

/// `POST /public/sign/<token>/decline` — the signer declines; the envelope
/// closes and the back office is notified.
#[rocket_okapi::openapi(tag = "E-Signature (Public)")]
#[post("/public/sign/<token>/decline", data = "<body>")]
pub async fn decline(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    client_ip: ClientIp,
    user_agent: UserAgent,
    token: &str,
    body: Json<DeclineReq>,
) -> ApiResult<Json<PublicSignView>> {
    let reason = body.into_inner().reason.filter(|r| !r.trim().is_empty());
    let (signer, envelope) = signer_for_token(&db, tenant.tenant_id, token).await?;
    if !super::is_open(&envelope.status) {
        return Err(ApiError::Conflict(format!(
            "this envelope is {} and can no longer be declined",
            envelope.status
        )));
    }
    if signer.status == "signed" {
        return Err(ApiError::Conflict(
            "you have already signed — contact the sender to void the envelope".into(),
        ));
    }

    let now = Utc::now();
    let mut am: entity::esign_signer::ActiveModel = signer.into();
    am.status = Set("declined".into());
    am.decline_reason = Set(reason.clone());
    am.updated_at = Set(now.into());
    let signer = am.update(&db).await?;

    // One decline closes the envelope; the document returns to draft.
    let mut em: entity::esign_envelope::ActiveModel = envelope.clone().into();
    em.status = Set("declined".into());
    em.updated_at = Set(now.into());
    let envelope = em.update(&db).await?;
    if let Some(doc) = LeaseDocument::find_by_id(envelope.lease_document_id)
        .filter(entity::lease_document::Column::TenantId.eq(tenant.tenant_id))
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
        tenant.tenant_id,
        envelope.id,
        Some(signer.id),
        "declined",
        json!({ "signer": signer.name, "reason": reason }),
        client_ip.0.clone(),
        user_agent.0.clone(),
    )
    .await;
    crate::audit::record(
        &db,
        None,
        crate::audit::actions::ESIGN_DECLINE,
        Some("esign_envelope"),
        Some(envelope.id.to_string()),
        Some(tenant.tenant_id),
        Some(json!({ "signer_id": signer.id, "reason": reason })),
    )
    .await;
    crate::notify::notify_staff(
        &db,
        tenant.tenant_id,
        "lease:read",
        "esign_declined_staff",
        json!({
            "signer": signer.name,
            "document_title": envelope.title,
            "reason_line": reason
                .as_deref()
                .map(|r| format!(" — reason: {r}"))
                .unwrap_or_default(),
        }),
        Some(("esign_envelope", envelope.id)),
        &format!("declined:{}", signer.id),
        None,
    )
    .await;

    Ok(Json(
        build_view(&db, tenant.tenant_id, signer, &envelope).await?,
    ))
}
