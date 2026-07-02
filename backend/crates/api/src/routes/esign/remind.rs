//! `POST /esign/envelopes/<id>/remind` — nudge every signer who hasn't signed
//! yet. Each pending signer's token is **rotated** (we never store raw tokens,
//! so a reminder mints a fresh link and the old one stops working).

use super::dto::{RemindResp, SignerLink};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::esign;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::EsignEnvelope;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::json;
use uuid::Uuid;

/// `POST /esign/envelopes/<id>/remind` — re-send signing links to pending signers.
#[rocket_okapi::openapi(tag = "E-Signature")]
#[post("/esign/envelopes/<id>/remind")]
pub async fn remind(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<RemindResp>> {
    user.require(Permission::LeaseManage)?;
    let eid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let envelope = EsignEnvelope::find_by_id(eid)
        .filter(entity::esign_envelope::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("envelope not found".into()))?;
    if !super::is_open(&envelope.status) {
        return Err(ApiError::Conflict(format!(
            "envelope is {} — nothing to remind",
            envelope.status
        )));
    }

    // Reminder count so far → distinct idempotency trigger per round.
    let round = super::envelope_events(&db, scope.tenant_id, eid)
        .await?
        .iter()
        .filter(|e| e.event == "reminded")
        .count()
        + 1;

    let slug = esign::tenant_slug(&db, scope.tenant_id).await;
    let signers = esign::envelope_signers(&db, scope.tenant_id, eid).await?;
    let now = Utc::now();
    let mut links = Vec::new();
    for s in signers {
        if !matches!(s.status.as_str(), "sent" | "viewed") {
            continue;
        }
        // Rotate the token: mint a fresh link, invalidate the old one.
        let (raw, hash) = esign::generate_token();
        let mut am: entity::esign_signer::ActiveModel = s.clone().into();
        am.token_hash = Set(hash);
        am.updated_at = Set(now.into());
        let saved = am.update(&db).await?;

        let sign_url = esign::sign_url(&slug, &raw);
        esign::record_event(
            &db,
            scope.tenant_id,
            eid,
            Some(saved.id),
            "reminded",
            json!({ "signer": saved.name, "round": round }),
            None,
            None,
        )
        .await;
        esign::notify_signer(
            &db,
            scope.tenant_id,
            &saved,
            "esign_reminder",
            &format!("reminder-{round}"),
            json!({
                "document_title": envelope.title,
                "sign_url": sign_url,
                "signer": saved.name,
            }),
        )
        .await;
        links.push(SignerLink {
            signer_id: saved.id,
            name: saved.name,
            email: saved.email,
            sign_url,
        });
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::ESIGN_REMIND,
        Some("esign_envelope"),
        Some(eid.to_string()),
        Some(scope.tenant_id),
        Some(json!({ "reminded": links.len(), "round": round })),
    )
    .await;

    Ok(Json(RemindResp {
        reminded: links.len(),
        sign_links: links,
    }))
}
