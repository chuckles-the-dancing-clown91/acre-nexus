use super::dto::{MortgageDto, UpdateMortgageReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Mortgage;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /mortgages/<id>` — update fields on a mortgage.
#[rocket_okapi::openapi(tag = "Financing")]
#[patch("/mortgages/<id>", data = "<body>")]
pub async fn update(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateMortgageReq>,
) -> ApiResult<Json<MortgageDto>> {
    user.require(Permission::FinanceManage)?;
    let mid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = Mortgage::find_by_id(mid)
        .filter(entity::mortgage::Column::TenantId.eq(scope.tenant_id))
        .one(&state.property_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("mortgage not found".into()))?;
    let b = body.into_inner();
    let mut am: entity::mortgage::ActiveModel = existing.into();
    if let Some(v) = b.lender_id {
        am.lender_id = Set(Some(v));
    }
    if let Some(v) = b.kind {
        am.kind = Set(v);
    }
    if let Some(v) = b.position {
        am.position = Set(v);
    }
    if let Some(v) = b.original_amount_cents {
        am.original_amount_cents = Set(Some(v));
    }
    if let Some(v) = b.current_balance_cents {
        am.current_balance_cents = Set(Some(v));
    }
    if let Some(v) = b.interest_rate_bps {
        am.interest_rate_bps = Set(Some(v));
    }
    if let Some(v) = b.term_months {
        am.term_months = Set(Some(v));
    }
    if let Some(v) = b.monthly_payment_cents {
        am.monthly_payment_cents = Set(Some(v));
    }
    if let Some(v) = b.escrow_monthly_cents {
        am.escrow_monthly_cents = Set(Some(v));
    }
    if let Some(v) = b.start_date {
        am.start_date = Set(Some(v));
    }
    if let Some(v) = b.maturity_date {
        am.maturity_date = Set(Some(v));
    }
    if let Some(v) = b.loan_number {
        am.loan_number = Set(Some(v));
    }
    if let Some(v) = b.status {
        am.status = Set(v);
    }
    if let Some(v) = b.notes {
        am.notes = Set(Some(v));
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&state.property_db).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::MORTGAGE_UPDATE,
        Some("mortgage"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "status": saved.status, "position": saved.position })),
    )
    .await;
    Ok(Json(MortgageDto::from(saved)))
}
