//! `POST /entities/<entity_id>/bank-accounts` — open an operating or trust
//! account for a legal entity. Trust accounts carry the commingling invariant
//! (enforced on postings in `crate::accounting`).

use super::dto::{BankAccountResp, CreateBankAccountReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::pii::last4;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Llc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use uuid::Uuid;

const VALID_KINDS: &[&str] = &["operating", "trust"];

/// `POST /entities/<entity_id>/bank-accounts` — add a bank account.
#[rocket_okapi::openapi(tag = "Legal Entities")]
#[post("/entities/<entity_id>/bank-accounts", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity_id: &str,
    body: Json<CreateBankAccountReq>,
) -> ApiResult<Json<BankAccountResp>> {
    user.require(Permission::FinanceManage)?;
    let eid =
        Uuid::parse_str(entity_id).map_err(|_| ApiError::BadRequest("invalid entity id".into()))?;
    let b = body.into_inner();
    if !VALID_KINDS.contains(&b.kind.as_str()) {
        return Err(ApiError::BadRequest(
            "kind must be 'operating' or 'trust'".into(),
        ));
    }
    if b.institution.trim().is_empty() {
        return Err(ApiError::BadRequest("institution is required".into()));
    }

    Llc::find_by_id(eid)
        .one(&db)
        .await?
        .filter(|l| l.tenant_id == scope.tenant_id)
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))?;

    let masked = b
        .account_number
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map(|n| format!("••••{}", last4(n)));

    let id = Uuid::new_v4();
    let saved = entity::bank_account::ActiveModel {
        id: Set(id),
        tenant_id: Set(scope.tenant_id),
        entity_id: Set(eid),
        kind: Set(b.kind.clone()),
        institution: Set(b.institution.trim().to_string()),
        masked_number: Set(masked),
        status: Set("active".into()),
        created_at: Set(Utc::now().into()),
        provider: Set(None),
        external_id: Set(None),
        last_synced_at: Set(None),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::BANK_ACCOUNT_CREATE,
        Some("bank_account"),
        Some(id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "entity_id": eid, "kind": b.kind })),
    )
    .await;
    Ok(Json(BankAccountResp::from(saved)))
}
