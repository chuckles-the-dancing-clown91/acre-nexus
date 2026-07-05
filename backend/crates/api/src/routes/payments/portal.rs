//! `/my/lease` + `/my/payments` + `/my/payment-methods` + `/my/autopay` —
//! the **renter portal's** payment surface. No staff permission required: all
//! data is scoped to the signed-in resident's own lease (matched by account
//! email, like `/my/applications`). Methods are tokenized only — a live
//! deployment passes a client-side Stripe token; the simulated tokenizer
//! mints `sim_pm_…` from display metadata.

use super::dto::{AddMethodReq, AutopayReq, MyLeaseResp, PayReq, PaymentDto, PaymentMethodDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{LeasePayment, PaymentMethod, Property, Unit};
use rocket::serde::json::Json;
use rocket::{delete, get, post, put, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

/// The signed-in resident's lease, or 404.
async fn my_lease(
    db: &crate::db::RequestDb,
    tenant_id: Uuid,
    user_id: Uuid,
) -> ApiResult<entity::lease::Model> {
    crate::payments::lease_for_user(db, tenant_id, user_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("no lease found for your account".into()))
}

/// `GET /my/lease` — the resident's lease with balance, payable items,
/// history, saved methods, and autopay state.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[get("/my/lease")]
pub async fn get_my_lease(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<MyLeaseResp>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let property = Property::find_by_id(lease.property_id)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?;
    let unit_label = match lease.unit_id {
        Some(uid) => Unit::find_by_id(uid)
            .filter(entity::unit::Column::TenantId.eq(scope.tenant_id))
            .one(&db)
            .await?
            .map(|u| u.unit_number),
        None => None,
    };

    let payments = LeasePayment::find()
        .filter(entity::lease_payment::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease_payment::Column::LeaseId.eq(lease.id))
        .order_by_desc(entity::lease_payment::Column::DueDate)
        .all(&db)
        .await?;
    let deposit_paid = payments.iter().any(|p| {
        p.kind == crate::payments::KIND_DEPOSIT
            && matches!(p.status.as_str(), "paid" | "processing")
    });
    let (due_items, history): (Vec<_>, Vec<_>) = payments
        .into_iter()
        .partition(|p| matches!(p.status.as_str(), "due" | "late" | "failed"));

    let methods = PaymentMethod::find()
        .filter(entity::payment_method::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::payment_method::Column::LeaseId.eq(lease.id))
        .filter(entity::payment_method::Column::Status.eq("active"))
        .order_by_asc(entity::payment_method::Column::CreatedAt)
        .all(&db)
        .await?;

    let autopay_enabled = crate::settings::get_bool(
        &db,
        scope.tenant_id,
        crate::settings::PAYMENTS_AUTOPAY_ENABLED,
    )
    .await;

    Ok(Json(MyLeaseResp {
        lease_id: lease.id,
        property_name: property
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default(),
        property_address: property.map(|p| p.address).unwrap_or_default(),
        unit_label,
        status: lease.status,
        payment_status: lease.payment_status,
        rent_cents: lease.rent_cents,
        rent_label: crate::dto::usd(lease.rent_cents),
        balance_cents: lease.balance_cents,
        balance_label: crate::dto::usd(lease.balance_cents),
        deposit_cents: lease.deposit_cents,
        deposit_label: lease.deposit_cents.map(crate::dto::usd),
        deposit_paid,
        autopay_enabled,
        due_items: due_items.into_iter().map(PaymentDto::from).collect(),
        history: history.into_iter().map(PaymentDto::from).collect(),
        methods: methods.into_iter().map(PaymentMethodDto::from).collect(),
    }))
}

/// `POST /my/payment-methods` — save a tokenized method for the lease.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/payment-methods", data = "<body>")]
pub async fn add_method(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<AddMethodReq>,
) -> ApiResult<Json<PaymentMethodDto>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let b = body.into_inner();
    if !matches!(b.kind.as_str(), "card" | "ach") {
        return Err(ApiError::BadRequest("kind must be card|ach".into()));
    }
    let last4: String = b
        .last4
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    let last4 = if last4.len() >= 4 {
        last4[last4.len() - 4..].to_string()
    } else if !last4.is_empty() {
        last4
    } else {
        "0000".into()
    };
    // Client-side tokens (Stripe.js) pass through; otherwise the simulated
    // tokenizer mints a stable token carrying only the display last4.
    let (provider, external_id) = match b.external_id.filter(|t| !t.trim().is_empty()) {
        Some(token) => ("stripe".to_string(), token),
        None => (
            "simulated".to_string(),
            format!("sim_pm_{}{}", Uuid::new_v4().simple(), last4),
        ),
    };

    let saved = entity::payment_method::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        lease_id: Set(Some(lease.id)),
        user_id: Set(Some(user.user_id)),
        provider: Set(provider),
        kind: Set(b.kind),
        external_id: Set(external_id),
        brand: Set(b.brand.filter(|s| !s.trim().is_empty())),
        last4: Set(last4),
        exp_month: Set(b.exp_month),
        exp_year: Set(b.exp_year),
        status: Set("active".into()),
        autopay: Set(false),
        autopay_day: Set(None),
        created_at: Set(Utc::now().into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PAYMENT_METHOD_ADD,
        Some("payment_method"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({
            "lease_id": lease.id,
            "kind": saved.kind,
            "last4": saved.last4,
        })),
    )
    .await;

    Ok(Json(PaymentMethodDto::from(saved)))
}

/// `DELETE /my/payment-methods/<id>` — remove a saved method (autopay
/// enrollment goes with it).
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[delete("/my/payment-methods/<id>")]
pub async fn remove_method(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let mid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let method = PaymentMethod::find_by_id(mid)
        .filter(entity::payment_method::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::payment_method::Column::LeaseId.eq(lease.id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("payment method not found".into()))?;

    let mut am: entity::payment_method::ActiveModel = method.into();
    am.status = Set("removed".into());
    am.autopay = Set(false);
    am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::PAYMENT_METHOD_REMOVE,
        Some("payment_method"),
        Some(mid.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": lease.id })),
    )
    .await;

    Ok(Json(serde_json::json!({ "removed": true })))
}

/// `POST /my/payments` — pay a due item in full (or raise + pay the security
/// deposit). The charge rides the durable payment pipeline; settlement
/// arrives via webhook (live) or after the simulated processor's delay.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[post("/my/payments", data = "<body>")]
pub async fn pay(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<PayReq>,
) -> ApiResult<Json<PaymentDto>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let b = body.into_inner();

    let method = PaymentMethod::find_by_id(b.method_id)
        .filter(entity::payment_method::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::payment_method::Column::LeaseId.eq(lease.id))
        .filter(entity::payment_method::Column::Status.eq("active"))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("payment method not found".into()))?;

    let payment = match (b.payment_id, b.kind.as_deref()) {
        (Some(pid), _) => LeasePayment::find_by_id(pid)
            .filter(entity::lease_payment::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::lease_payment::Column::LeaseId.eq(lease.id))
            .one(&db)
            .await?
            .ok_or_else(|| ApiError::NotFound("payment not found".into()))?,
        (None, Some("deposit")) => {
            let amount = lease
                .deposit_cents
                .filter(|d| *d > 0)
                .ok_or_else(|| ApiError::BadRequest("this lease has no deposit".into()))?;
            let existing = LeasePayment::find()
                .filter(entity::lease_payment::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::lease_payment::Column::LeaseId.eq(lease.id))
                .filter(entity::lease_payment::Column::Kind.eq(crate::payments::KIND_DEPOSIT))
                .one(&db)
                .await?;
            match existing {
                // A failed prior attempt is retryable; anything else means the
                // deposit is already in flight or settled.
                Some(p) if p.status == "failed" => p,
                Some(p) => {
                    return Err(ApiError::BadRequest(format!(
                        "deposit already {}",
                        p.status
                    )))
                }
                None => {
                    entity::lease_payment::ActiveModel {
                        id: Set(Uuid::new_v4()),
                        tenant_id: Set(scope.tenant_id),
                        lease_id: Set(lease.id),
                        due_date: Set(Utc::now().date_naive().to_string()),
                        amount_cents: Set(amount),
                        paid_date: Set(None),
                        status: Set("due".into()),
                        method: Set(None),
                        created_at: Set(Utc::now().into()),
                        kind: Set(crate::payments::KIND_DEPOSIT.into()),
                        method_id: Set(None),
                        provider: Set(None),
                        external_id: Set(None),
                        failure_reason: Set(None),
                        receipt_number: Set(None),
                        ledger_txn_id: Set(None),
                    }
                    .insert(&db)
                    .await?
                }
            }
        }
        _ => {
            return Err(ApiError::BadRequest(
                "pass payment_id (a due item) or kind: \"deposit\"".into(),
            ))
        }
    };

    let saved =
        crate::payments::start_charge(&db, scope.tenant_id, payment, &method, Some(user.user_id))
            .await?;
    Ok(Json(PaymentDto::from(saved)))
}

/// `PUT /my/autopay` — enroll one saved method in autopay for the lease.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[put("/my/autopay", data = "<body>")]
pub async fn set_autopay(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<AutopayReq>,
) -> ApiResult<Json<PaymentMethodDto>> {
    if !crate::settings::get_bool(
        &db,
        scope.tenant_id,
        crate::settings::PAYMENTS_AUTOPAY_ENABLED,
    )
    .await
    {
        return Err(ApiError::Forbidden(
            "autopay is not enabled for this workspace".into(),
        ));
    }
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let b = body.into_inner();
    let method = PaymentMethod::find_by_id(b.method_id)
        .filter(entity::payment_method::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::payment_method::Column::LeaseId.eq(lease.id))
        .filter(entity::payment_method::Column::Status.eq("active"))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("payment method not found".into()))?;

    // One autopay method per lease: clear any existing enrollment first.
    let existing = PaymentMethod::find()
        .filter(entity::payment_method::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::payment_method::Column::LeaseId.eq(lease.id))
        .filter(entity::payment_method::Column::Autopay.eq(true))
        .all(&db)
        .await?;
    for prior in existing {
        if prior.id == method.id {
            continue;
        }
        let mut am: entity::payment_method::ActiveModel = prior.into();
        am.autopay = Set(false);
        am.autopay_day = Set(None);
        am.update(&db).await?;
    }

    let day = match b.day {
        Some(d) => d.clamp(1, 28),
        None => {
            crate::settings::get_i64(&db, scope.tenant_id, crate::settings::PAYMENTS_RENT_DUE_DAY)
                .await
                .clamp(1, 28) as i32
        }
    };
    let mut am: entity::payment_method::ActiveModel = method.into();
    am.autopay = Set(true);
    am.autopay_day = Set(Some(day));
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::AUTOPAY_ENROLL,
        Some("payment_method"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "lease_id": lease.id, "day": day })),
    )
    .await;

    Ok(Json(PaymentMethodDto::from(saved)))
}

/// `DELETE /my/autopay` — cancel the lease's autopay enrollment.
#[rocket_okapi::openapi(tag = "Renter Portal")]
#[delete("/my/autopay")]
pub async fn cancel_autopay(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<serde_json::Value>> {
    let lease = my_lease(&db, scope.tenant_id, user.user_id).await?;
    let enrolled = PaymentMethod::find()
        .filter(entity::payment_method::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::payment_method::Column::LeaseId.eq(lease.id))
        .filter(entity::payment_method::Column::Autopay.eq(true))
        .all(&db)
        .await?;
    let had_any = !enrolled.is_empty();
    for method in enrolled {
        let id = method.id;
        let mut am: entity::payment_method::ActiveModel = method.into();
        am.autopay = Set(false);
        am.autopay_day = Set(None);
        am.update(&db).await?;
        crate::audit::record(
            &db,
            Some(user.user_id),
            crate::audit::actions::AUTOPAY_CANCEL,
            Some("payment_method"),
            Some(id.to_string()),
            Some(scope.tenant_id),
            Some(serde_json::json!({ "lease_id": lease.id })),
        )
        .await;
    }
    Ok(Json(serde_json::json!({ "cancelled": had_any })))
}
