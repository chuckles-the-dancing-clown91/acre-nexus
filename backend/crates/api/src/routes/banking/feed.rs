//! Bank **feed + reconciliation** endpoints (Phase 3): link an account for
//! feeds, trigger a sync, browse feed transactions, and match/ignore lines.

use super::dto::{BankAccountResp, BankTxnDto, LinkAccountReq, MatchTxnReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::providers::bank::{BankRequest, BankResponse, PlaidProvider};
use crate::providers::{Provider, ProviderCtx};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{BankAccount, BankTxn, LeasePayment};
use rocket::serde::json::Json;
use rocket::{get, post, State};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

async fn account_for(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::bank_account::Model> {
    let aid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    BankAccount::find_by_id(aid)
        .filter(entity::bank_account::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("bank account not found".into()))
}

/// `POST /bank-accounts/<id>/link` — link the account for feeds (Plaid Link
/// token in live mode; the simulator mints a stable account id).
#[rocket_okapi::openapi(tag = "Banking")]
#[post("/bank-accounts/<id>/link", data = "<body>")]
pub async fn link(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<LinkAccountReq>,
) -> ApiResult<Json<BankAccountResp>> {
    user.require(Permission::PaymentManage)?;
    let account = account_for(&db, scope.tenant_id, id).await?;

    let ctx = ProviderCtx::new(&db, scope.tenant_id);
    let req = BankRequest::Link {
        bank_account_id: account.id,
        institution: account.institution.clone(),
        public_token: body.into_inner().public_token,
    };
    let resp = PlaidProvider
        .execute(&ctx, &req)
        .await
        .map_err(|e| ApiError::BadRequest(format!("bank link failed: {e}")))?;
    let BankResponse::Linked {
        account_external_id,
    } = resp
    else {
        return Err(ApiError::BadRequest("unexpected provider response".into()));
    };

    let mut am: entity::bank_account::ActiveModel = account.into();
    am.provider = Set(Some("plaid".into()));
    am.external_id = Set(Some(account_external_id));
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::BANK_ACCOUNT_LINK,
        Some("bank_account"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "institution": saved.institution })),
    )
    .await;

    // First sync right away.
    crate::scheduler::enqueue(
        &db,
        scope.tenant_id,
        "bank_feed_sync",
        serde_json::json!({ "bank_account_id": saved.id }),
        0,
    )
    .await?;

    Ok(Json(BankAccountResp::from(saved)))
}

/// `POST /bank-accounts/<id>/sync` — pull the feed now (rides the queue).
#[rocket_okapi::openapi(tag = "Banking")]
#[post("/bank-accounts/<id>/sync")]
pub async fn sync(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::PaymentManage)?;
    let account = account_for(&db, scope.tenant_id, id).await?;
    if account.external_id.is_none() {
        return Err(ApiError::BadRequest(
            "link the account for feeds first".into(),
        ));
    }
    let job_id = crate::scheduler::enqueue(
        &db,
        scope.tenant_id,
        "bank_feed_sync",
        serde_json::json!({ "bank_account_id": account.id }),
        0,
    )
    .await?;
    Ok(Json(
        serde_json::json!({ "queued": true, "job_id": job_id }),
    ))
}

/// `GET /bank-accounts/<id>/transactions?status=` — the account's feed lines,
/// newest first.
#[rocket_okapi::openapi(tag = "Banking")]
#[get("/bank-accounts/<id>/transactions?<status>")]
pub async fn transactions(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    status: Option<String>,
) -> ApiResult<Json<Vec<BankTxnDto>>> {
    user.require(Permission::PaymentRead)?;
    let account = account_for(&db, scope.tenant_id, id).await?;
    let mut q = BankTxn::find()
        .filter(entity::bank_txn::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::bank_txn::Column::BankAccountId.eq(account.id));
    if let Some(status) = status.filter(|s| !s.trim().is_empty()) {
        q = q.filter(entity::bank_txn::Column::Status.eq(status));
    }
    let rows = q
        .order_by_desc(entity::bank_txn::Column::PostedDate)
        .limit(200)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(BankTxnDto::from).collect()))
}

/// `POST /bank-transactions/<id>/match` — manually reconcile a feed line
/// against a settled payment (amounts must agree).
#[rocket_okapi::openapi(tag = "Banking")]
#[post("/bank-transactions/<id>/match", data = "<body>")]
pub async fn match_txn(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<MatchTxnReq>,
) -> ApiResult<Json<BankTxnDto>> {
    user.require(Permission::PaymentManage)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let txn = BankTxn::find_by_id(tid)
        .filter(entity::bank_txn::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("bank transaction not found".into()))?;
    let b = body.into_inner();
    let payment = LeasePayment::find_by_id(b.payment_id)
        .filter(entity::lease_payment::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("payment not found".into()))?;
    if payment.status != "paid" {
        return Err(ApiError::BadRequest(
            "only settled payments can reconcile".into(),
        ));
    }
    if payment.amount_cents != txn.amount_cents {
        return Err(ApiError::BadRequest(format!(
            "amounts differ: bank line {} vs payment {}",
            txn.amount_cents, payment.amount_cents
        )));
    }
    let already = BankTxn::find()
        .filter(entity::bank_txn::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::bank_txn::Column::MatchedPaymentId.eq(payment.id))
        .filter(entity::bank_txn::Column::Id.ne(txn.id))
        .one(&db)
        .await?;
    if already.is_some() {
        return Err(ApiError::Conflict(
            "that payment is already matched to another bank line".into(),
        ));
    }

    let mut am: entity::bank_txn::ActiveModel = txn.into();
    am.status = Set("matched".into());
    am.matched_payment_id = Set(Some(payment.id));
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::BANK_TXN_MATCH,
        Some("bank_txn"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "payment_id": payment.id, "auto": false })),
    )
    .await;

    Ok(Json(BankTxnDto::from(saved)))
}

/// `POST /bank-transactions/<id>/ignore` — park noise (bank fees, unrelated
/// transfers) out of the reconciliation queue.
#[rocket_okapi::openapi(tag = "Banking")]
#[post("/bank-transactions/<id>/ignore")]
pub async fn ignore_txn(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<BankTxnDto>> {
    user.require(Permission::PaymentManage)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let txn = BankTxn::find_by_id(tid)
        .filter(entity::bank_txn::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("bank transaction not found".into()))?;
    if txn.status == "matched" {
        return Err(ApiError::BadRequest(
            "unmatch is not supported; matched lines stay reconciled".into(),
        ));
    }
    let mut am: entity::bank_txn::ActiveModel = txn.into();
    am.status = Set("ignored".into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::BANK_TXN_MATCH,
        Some("bank_txn"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "ignored": true })),
    )
    .await;

    Ok(Json(BankTxnDto::from(saved)))
}

/// `GET /bank-accounts?entity=` — all of the tenant's bank accounts (the
/// per-entity listing already exists under `/entities/<id>/bank-accounts`;
/// reconciliation wants them all in one view).
#[rocket_okapi::openapi(tag = "Banking")]
#[get("/bank-accounts?<entity>")]
pub async fn list_all(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    entity: Option<String>,
) -> ApiResult<Json<Vec<BankAccountResp>>> {
    user.require(Permission::PaymentRead)?;
    let mut q =
        BankAccount::find().filter(entity::bank_account::Column::TenantId.eq(scope.tenant_id));
    if let Some(eid) = entity.and_then(|e| Uuid::parse_str(&e).ok()) {
        q = q.filter(entity::bank_account::Column::EntityId.eq(eid));
    }
    let rows = q
        .order_by_asc(entity::bank_account::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(BankAccountResp::from).collect()))
}
