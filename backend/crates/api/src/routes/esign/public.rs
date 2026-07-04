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
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set};
use serde_json::json;

/// Resolve a raw signing token to its (signer, envelope) pair.
///
/// `lock_envelope` takes `SELECT … FOR UPDATE` on the envelope row — the
/// mutation handlers (sign/decline) use it so two signers submitting at the
/// same moment serialize on the envelope: the second waits for the first's
/// commit and then sees its signature, which is what makes the "was that the
/// last signature?" check race-free.
async fn signer_for_token(
    db: &crate::db::RequestDb,
    tenant_id: uuid::Uuid,
    token: &str,
    lock_envelope: bool,
) -> ApiResult<(entity::esign_signer::Model, entity::esign_envelope::Model)> {
    let hash = esign::hash_token(token);
    let signer = EsignSigner::find()
        .filter(entity::esign_signer::Column::TenantId.eq(tenant_id))
        .filter(entity::esign_signer::Column::TokenHash.eq(hash))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("signing link is invalid or has expired".into()))?;
    let mut q = EsignEnvelope::find_by_id(signer.envelope_id)
        .filter(entity::esign_envelope::Column::TenantId.eq(tenant_id));
    if lock_envelope {
        q = q.lock_exclusive();
    }
    let envelope = q
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("envelope not found".into()))?;
    // Workspace-configurable link validity: 0 (the default) means links live
    // as long as the envelope; a positive window kills them N days after the
    // envelope was sent (staff void + re-send to issue fresh ones).
    let expiry_days =
        crate::settings::get_i64(db, tenant_id, crate::settings::ESIGN_LINK_EXPIRY_DAYS).await;
    if expiry_days > 0
        && super::is_open(&envelope.status)
        && Utc::now() - chrono::Duration::days(expiry_days) > envelope.created_at
    {
        return Err(ApiError::NotFound(
            "this signing link has expired — contact the sender for a fresh one".into(),
        ));
    }
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

/// `GET /public/sign/<token>` — the signer's view of the document. Read-only:
/// email link scanners and previewers fetch links before the human ever opens
/// them, so "viewed" is recorded by [`mark_viewed`] on the signer's first real
/// interaction with the page instead of on fetch — keeping the ESIGN audit
/// trail's first-view entry a human act.
#[rocket_okapi::openapi(tag = "E-Signature (Public)")]
#[get("/public/sign/<token>")]
pub async fn view(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    token: &str,
) -> ApiResult<Json<PublicSignView>> {
    let (signer, envelope) = signer_for_token(&db, tenant.tenant_id, token, false).await?;
    Ok(Json(
        build_view(&db, tenant.tenant_id, signer, &envelope).await?,
    ))
}

/// `POST /public/sign/<token>/viewed` — the signing page calls this on the
/// signer's first interaction (not on load): sent → viewed, with IP + user
/// agent in the audit trail. Idempotent past the first call.
#[rocket_okapi::openapi(tag = "E-Signature (Public)")]
#[post("/public/sign/<token>/viewed")]
pub async fn mark_viewed(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    tenant: PublicTenant,
    client_ip: ClientIp,
    user_agent: UserAgent,
    token: &str,
) -> ApiResult<Json<PublicSignView>> {
    let (signer, envelope) = signer_for_token(&db, tenant.tenant_id, token, false).await?;
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

    let (signer, envelope) = signer_for_token(&db, tenant.tenant_id, token, true).await?;
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
    // The document may have been signed outside this envelope (in person)
    // while the link was out — never let a stale link overwrite that record.
    if let Some(doc) = LeaseDocument::find_by_id(envelope.lease_document_id)
        .filter(entity::lease_document::Column::TenantId.eq(tenant.tenant_id))
        .one(&db)
        .await?
    {
        if doc.status == "signed" {
            return Err(ApiError::Conflict(
                "this document has already been signed — no further signatures are needed".into(),
            ));
        }
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
        esign::complete_envelope(&db, &envelope, &signers).await?
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
    let (signer, envelope) = signer_for_token(&db, tenant.tenant_id, token, true).await?;
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
    // The deal died from the resident's side — the advertised listing (if the
    // lease came from one) goes back on the market.
    crate::listing_sync::reopen_on_deal_death(&db, tenant.tenant_id, envelope.lease_id).await;

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
