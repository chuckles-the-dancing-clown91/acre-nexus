//! `POST /leases/<id>/envelope` — send the lease's latest generated document
//! out for signature: create the envelope + signers, mint one-time signing
//! links, and queue the email/SMS notifications.

use super::dto::{CreateEnvelopeReq, CreateEnvelopeResp, EnvelopeDto, SignerLink};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::esign::{self, NewSigner};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{EsignEnvelope, Lease, LeaseDocument, User};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

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
        // The lease agreement itself — a renewal addendum is sent through the
        // dedicated renewals flow, not this route.
        .filter(entity::lease_document::Column::Purpose.eq("lease"))
        .order_by_desc(entity::lease_document::Column::GeneratedAt)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("generate a document before sending".into()))?;
    if doc.status == "signed" {
        return Err(ApiError::Conflict("document is already signed".into()));
    }

    // One lease-agreement envelope out at a time per lease — void the old one
    // to re-send. (Renewal addenda ride their own envelopes, tracked apart.)
    let open = EsignEnvelope::find()
        .filter(entity::esign_envelope::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::esign_envelope::Column::LeaseId.eq(lid))
        .filter(entity::esign_envelope::Column::Purpose.eq("lease"))
        .filter(entity::esign_envelope::Column::Status.is_in(["sent", "partially_signed"]))
        .one(&db)
        .await?;
    if open.is_some() {
        return Err(ApiError::Conflict(
            "an envelope is already out for signature on this lease — void it first".into(),
        ));
    }

    let signers = resolve_signers(&db, &lease, &user, b.signers).await?;

    let (envelope, saved_signers, raw_links) = esign::issue_envelope(
        &db,
        scope.tenant_id,
        user.user_id,
        &lease,
        &doc,
        "lease",
        b.message,
        signers,
    )
    .await?;

    let links = SignerLink::from_pairs(&saved_signers, &raw_links);
    let events = super::envelope_events(&db, scope.tenant_id, envelope.id).await?;
    Ok(Json(CreateEnvelopeResp {
        envelope: EnvelopeDto::build(envelope, saved_signers, events),
        sign_links: links,
    }))
}

/// Validate explicit signers, or derive the defaults (the lease's resident +
/// the sending user as landlord).
pub(crate) async fn resolve_signers(
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
        None => default_signers(db, lease, user).await?,
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

/// The default signers for a lease document: the resident plus the acting user
/// as landlord. Shared by the initial-signing and renewal send flows.
pub(crate) async fn default_signers(
    db: &crate::db::RequestDb,
    lease: &entity::lease::Model,
    user: &AuthUser,
) -> ApiResult<Vec<NewSigner>> {
    let resident_email = lease.tenant_email.clone().ok_or_else(|| {
        ApiError::BadRequest(
            "lease has no resident email — add one or specify signers explicitly".into(),
        )
    })?;
    let sender = User::find_by_id(user.user_id)
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("sending user not found".into()))?;
    Ok(vec![
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
    ])
}
