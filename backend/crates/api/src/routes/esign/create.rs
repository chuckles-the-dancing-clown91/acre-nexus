//! `POST /leases/<id>/envelope` — send the lease's latest generated document
//! out for signature: create the envelope + signers, mint one-time signing
//! links, and queue the email/SMS notifications.

use super::dto::{CreateEnvelopeReq, CreateEnvelopeResp, EnvelopeDto, SignerLink};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::esign;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{EsignEnvelope, Lease, LeaseDocument, User};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::json;
use uuid::Uuid;

/// A signer as validated from the request (or derived from the lease).
struct NewSigner {
    role: String,
    name: String,
    email: String,
    phone: Option<String>,
}

/// `POST /leases/<id>/envelope` — create + send an e-signature envelope for
/// the lease's latest generated document.
#[rocket_okapi::openapi(tag = "E-Signature")]
#[post("/leases/<id>/envelope", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateEnvelopeReq>,
) -> ApiResult<Json<CreateEnvelopeResp>> {
    user.require(Permission::LeaseManage)?;
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let b = body.into_inner();

    let lease = Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))?;
    let doc = LeaseDocument::find()
        .filter(entity::lease_document::Column::LeaseId.eq(lid))
        .filter(entity::lease_document::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::lease_document::Column::GeneratedAt)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("generate a document before sending".into()))?;
    if doc.status == "signed" {
        return Err(ApiError::Conflict("document is already signed".into()));
    }

    // One envelope out at a time per lease — void the old one to re-send.
    let open = EsignEnvelope::find()
        .filter(entity::esign_envelope::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::esign_envelope::Column::LeaseId.eq(lid))
        .filter(entity::esign_envelope::Column::Status.is_in(["sent", "partially_signed"]))
        .one(&db)
        .await?;
    if open.is_some() {
        return Err(ApiError::Conflict(
            "an envelope is already out for signature on this lease — void it first".into(),
        ));
    }

    let signers = resolve_signers(&db, &lease, &user, b.signers).await?;

    // Pin the exact text being signed.
    let body_hash = crate::storage::sha256_hex(doc.body.as_bytes());
    let now = Utc::now();
    let envelope = entity::esign_envelope::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(lid),
        lease_document_id: Set(doc.id),
        title: Set(doc.title.clone()),
        message: Set(b.message.clone().filter(|m| !m.trim().is_empty())),
        status: Set("sent".into()),
        body_hash: Set(body_hash),
        signed_document_id: Set(None),
        created_by: Set(Some(user.user_id)),
        sent_at: Set(now.into()),
        completed_at: Set(None),
        voided_at: Set(None),
        void_reason: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    // The document is now out for signature.
    if doc.status != "sent" {
        let mut dm: entity::lease_document::ActiveModel = doc.into();
        dm.status = Set("sent".into());
        dm.update(&db).await?;
    }

    let slug = esign::tenant_slug(&db, scope.tenant_id).await;
    let mut links: Vec<SignerLink> = Vec::with_capacity(signers.len());
    let mut saved_signers = Vec::with_capacity(signers.len());
    for s in signers {
        let (raw, hash) = esign::generate_token();
        let (token_ciphertext, token_nonce) = esign::seal_token(&raw)?;
        let saved = entity::esign_signer::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            envelope_id: Set(envelope.id),
            role: Set(s.role),
            name: Set(s.name),
            email: Set(s.email),
            phone: Set(s.phone),
            token_hash: Set(hash),
            token_ciphertext: Set(token_ciphertext),
            token_nonce: Set(token_nonce),
            status: Set("sent".into()),
            viewed_at: Set(None),
            signed_at: Set(None),
            signed_name: Set(None),
            signed_ip: Set(None),
            signed_user_agent: Set(None),
            decline_reason: Set(None),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&db)
        .await?;

        let sign_url = esign::sign_url(&slug, &raw);
        esign::record_event(
            &db,
            scope.tenant_id,
            envelope.id,
            Some(saved.id),
            "sent",
            json!({ "signer": saved.name, "email": saved.email, "role": saved.role }),
            None,
            None,
        )
        .await;
        esign::notify_signer(
            &db,
            scope.tenant_id,
            &saved,
            "esign_request",
            "request",
            json!({
                "document_title": envelope.title,
                "sign_url": sign_url,
                "signer": saved.name,
                "message": envelope.message.clone().unwrap_or_default(),
            }),
        )
        .await;

        links.push(SignerLink {
            signer_id: saved.id,
            name: saved.name.clone(),
            email: saved.email.clone(),
            sign_url,
        });
        saved_signers.push(saved);
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ESIGN_SEND,
        Some("esign_envelope"),
        Some(envelope.id.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "lease_id": lid, "signers": saved_signers.len() })),
    )
    .await;

    let events = super::envelope_events(&db, scope.tenant_id, envelope.id).await?;
    Ok(Json(CreateEnvelopeResp {
        envelope: EnvelopeDto::build(envelope, saved_signers, events),
        sign_links: links,
    }))
}

/// Validate explicit signers, or derive the defaults (the lease's resident +
/// the sending user as landlord).
async fn resolve_signers(
    db: &crate::db::RequestDb,
    lease: &entity::lease::Model,
    user: &AuthUser,
    explicit: Option<Vec<super::dto::SignerReq>>,
) -> ApiResult<Vec<NewSigner>> {
    let signers: Vec<NewSigner> = match explicit {
        Some(list) => list
            .into_iter()
            .map(|s| {
                let role = s.role.unwrap_or_else(|| "other".into());
                if !esign::SIGNER_ROLES.contains(&role.as_str()) {
                    return Err(ApiError::BadRequest(format!(
                        "invalid signer role '{role}' (expected one of {})",
                        esign::SIGNER_ROLES.join(", ")
                    )));
                }
                let name = s.name.trim().to_string();
                let email = s.email.trim().to_lowercase();
                if name.is_empty() {
                    return Err(ApiError::BadRequest("signer name is required".into()));
                }
                if !email.contains('@') {
                    return Err(ApiError::BadRequest(format!(
                        "invalid signer email '{email}'"
                    )));
                }
                Ok(NewSigner {
                    role,
                    name,
                    email,
                    phone: s.phone.filter(|p| !p.trim().is_empty()),
                })
            })
            .collect::<Result<_, _>>()?,
        None => {
            let resident_email = lease.tenant_email.clone().ok_or_else(|| {
                ApiError::BadRequest(
                    "lease has no resident email — add one or specify signers explicitly".into(),
                )
            })?;
            let sender = User::find_by_id(user.user_id)
                .one(db)
                .await?
                .ok_or_else(|| ApiError::NotFound("sending user not found".into()))?;
            vec![
                NewSigner {
                    role: "resident".into(),
                    name: lease.tenant_name.clone(),
                    email: resident_email.trim().to_lowercase(),
                    phone: lease.tenant_phone.clone().filter(|p| !p.trim().is_empty()),
                },
                NewSigner {
                    role: "landlord".into(),
                    name: sender.name,
                    email: sender.email,
                    phone: None,
                },
            ]
        }
    };
    let max_signers =
        crate::settings::get_i64(db, lease.tenant_id, crate::settings::ESIGN_MAX_SIGNERS)
            .await
            .max(1) as usize;
    if signers.is_empty() || signers.len() > max_signers {
        return Err(ApiError::BadRequest(format!(
            "an envelope needs between 1 and {max_signers} signers"
        )));
    }
    Ok(signers)
}
