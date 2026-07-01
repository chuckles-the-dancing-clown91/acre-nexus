use super::dto::{CreateMortgageReq, MortgageDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /properties/<id>/mortgages` — attach a new mortgage/loan to a property.
#[rocket_okapi::openapi(tag = "Financing")]
#[post("/properties/<id>/mortgages", data = "<body>")]
pub async fn create(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<CreateMortgageReq>,
) -> ApiResult<Json<MortgageDto>> {
    user.require(Permission::FinanceManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let b = body.into_inner();
    let now = Utc::now();
    let kind = if b.kind.trim().is_empty() {
        "purchase".to_string()
    } else {
        b.kind
    };
    let status = match b.status {
        Some(s) if !s.trim().is_empty() => s,
        _ => "active".to_string(),
    };
    let model = entity::mortgage::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        property_id: Set(pid),
        lender_id: Set(b.lender_id),
        kind: Set(kind),
        position: Set(b.position.unwrap_or(1)),
        original_amount_cents: Set(b.original_amount_cents),
        current_balance_cents: Set(b.current_balance_cents),
        interest_rate_bps: Set(b.interest_rate_bps),
        term_months: Set(b.term_months),
        monthly_payment_cents: Set(b.monthly_payment_cents),
        escrow_monthly_cents: Set(b.escrow_monthly_cents),
        start_date: Set(b.start_date),
        maturity_date: Set(b.maturity_date),
        loan_number: Set(b.loan_number),
        status: Set(status),
        notes: Set(b.notes),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    };
    let saved = model.insert(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::MORTGAGE_CREATE,
        Some("mortgage"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "property_id": saved.property_id, "kind": saved.kind, "position": saved.position })),
    )
    .await;
    Ok(Json(MortgageDto::from(saved)))
}
