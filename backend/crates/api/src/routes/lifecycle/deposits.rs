//! Security-deposit disposition routes. Staff draft deductions with
//! `lease:manage` and finalize (moving real money) with `payout:manage`;
//! residents see their own disposition through `GET /my/deposit`.

use super::dto::{disposition_dto, DispositionDto, LeaseDepositResp, UpsertDispositionReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{DepositDisposition, Lease};
use rocket::serde::json::Json;
use rocket::{get, post, put, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, Set};
use uuid::Uuid;

/// A tenant-scoped lease, or 404.
async fn find_lease(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    id: &str,
) -> ApiResult<entity::lease::Model> {
    let lid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Lease::find_by_id(lid)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| ApiError::NotFound("lease not found".into()))
}

/// The lease's disposition, if drafted.
async fn disposition_for_lease(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    lease_id: Uuid,
) -> ApiResult<Option<entity::deposit_disposition::Model>> {
    Ok(DepositDisposition::find()
        .filter(entity::deposit_disposition::Column::TenantId.eq(tenant_id))
        .filter(entity::deposit_disposition::Column::LeaseId.eq(lease_id))
        .one(db)
        .await?)
}

async fn dto_for(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    d: entity::deposit_disposition::Model,
) -> ApiResult<DispositionDto> {
    let lines = crate::deposits::deductions(db, tenant_id, d.id).await?;
    Ok(disposition_dto(d, lines))
}

/// Build the full deposit picture for one lease.
async fn deposit_resp(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    lease: &entity::lease::Model,
) -> ApiResult<LeaseDepositResp> {
    let paid = crate::deposits::deposit_settled(db, tenant_id, lease.id).await?;
    let disposition = match disposition_for_lease(db, tenant_id, lease.id).await? {
        Some(d) => Some(dto_for(db, tenant_id, d).await?),
        None => None,
    };
    Ok(LeaseDepositResp {
        lease_id: lease.id,
        deposit_cents: lease.deposit_cents,
        deposit_label: lease.deposit_cents.map(crate::dto::usd),
        deposit_paid: paid,
        disposition,
    })
}

/// `GET /leases/<id>/deposit` — the lease's deposit status + disposition.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[get("/leases/<id>/deposit")]
pub async fn get_lease_deposit(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<LeaseDepositResp>> {
    user.require(Permission::LeaseRead)?;
    let lease = find_lease(&db, scope.tenant_id, id).await?;
    Ok(Json(deposit_resp(&db, scope.tenant_id, &lease).await?))
}

/// `PUT /leases/<id>/deposit/disposition` — create or replace the **draft**
/// disposition: itemized deductions + notes. Finalize moves the money.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[put("/leases/<id>/deposit/disposition", data = "<body>")]
pub async fn upsert_disposition(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpsertDispositionReq>,
) -> ApiResult<Json<DispositionDto>> {
    user.require(Permission::LeaseManage)?;
    let lease = find_lease(&db, scope.tenant_id, id).await?;
    let deposit = lease
        .deposit_cents
        .filter(|d| *d > 0)
        .ok_or_else(|| ApiError::BadRequest("this lease has no deposit".into()))?;
    if !crate::deposits::deposit_settled(&db, scope.tenant_id, lease.id).await? {
        return Err(ApiError::BadRequest(
            "the deposit has not settled into trust for this lease".into(),
        ));
    }

    let b = body.into_inner();
    // Validate the whole set before writing anything.
    let amounts: Vec<i64> = b.deductions.iter().map(|d| d.amount_cents).collect();
    crate::deposits::compute_refund(deposit, &amounts)?;
    for d in &b.deductions {
        if d.description.trim().is_empty() {
            return Err(ApiError::BadRequest(
                "every deduction needs a description".into(),
            ));
        }
    }

    let now = Utc::now();
    let existing = disposition_for_lease(&db, scope.tenant_id, lease.id).await?;
    let was_new = existing.is_none();
    let disposition = match existing {
        Some(d) if d.status == "draft" => {
            // Replace the draft's deductions wholesale.
            for line in crate::deposits::deductions(&db, scope.tenant_id, d.id).await? {
                line.delete(&db).await?;
            }
            let mut am: entity::deposit_disposition::ActiveModel = d.into();
            am.deposit_cents = Set(deposit);
            am.notes = Set(b.notes.clone().filter(|n| !n.trim().is_empty()));
            am.updated_at = Set(now.into());
            am.update(&db).await?
        }
        Some(d) => {
            return Err(ApiError::BadRequest(format!(
                "disposition is no longer editable (status: {})",
                d.status
            )))
        }
        None => {
            entity::deposit_disposition::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(scope.tenant_id),
                lease_id: Set(lease.id),
                property_id: Set(lease.property_id),
                status: Set("draft".into()),
                deposit_cents: Set(deposit),
                refund_cents: Set(None),
                notes: Set(b.notes.clone().filter(|n| !n.trim().is_empty())),
                provider: Set(None),
                external_id: Set(None),
                failure_reason: Set(None),
                statement_document_id: Set(None),
                finalized_by: Set(None),
                finalized_at: Set(None),
                closed_at: Set(None),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(&db)
            .await?
        }
    };

    for (idx, d) in b.deductions.iter().enumerate() {
        entity::deposit_deduction::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(scope.tenant_id),
            disposition_id: Set(disposition.id),
            description: Set(d.description.trim().to_string()),
            amount_cents: Set(d.amount_cents),
            sort_order: Set(idx as i32),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await?;
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        if was_new {
            crate::audit::actions::DEPOSIT_DISPOSITION_CREATE
        } else {
            crate::audit::actions::DEPOSIT_DISPOSITION_UPDATE
        },
        Some("deposit_disposition"),
        Some(disposition.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "lease_id": lease.id,
            "deductions": b.deductions.len(),
        })),
    )
    .await;

    Ok(Json(dto_for(&db, scope.tenant_id, disposition).await?))
}

/// `POST /deposit-dispositions/<id>/finalize` — post the withheld deductions
/// to the ledger and kick the refund transfer (or settle immediately when
/// nothing refunds). Retries a failed refund.
#[rocket_okapi::openapi(tag = "Lifecycle")]
#[post("/deposit-dispositions/<id>/finalize")]
pub async fn finalize_disposition(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<DispositionDto>> {
    user.require(Permission::PayoutManage)?;
    let did = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let disposition = DepositDisposition::find_by_id(did)
        .filter(entity::deposit_disposition::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("disposition not found".into()))?;

    let saved = crate::deposits::finalize(&db, scope.tenant_id, disposition, user.user_id).await?;
    Ok(Json(dto_for(&db, scope.tenant_id, saved).await?))
}

/// `GET /my/deposit` — the resident's own deposit picture: amount, paid
/// state, and — after move-out — the disposition with its statement.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/deposit")]
pub async fn my_deposit(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<LeaseDepositResp>> {
    // Past leases count too — a moved-out resident reads their statement.
    let lease = crate::payments::any_lease_for_user(&db, scope.tenant_id, user.user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("no lease found for your account".into()))?;
    Ok(Json(deposit_resp(&db, scope.tenant_id, &lease).await?))
}
