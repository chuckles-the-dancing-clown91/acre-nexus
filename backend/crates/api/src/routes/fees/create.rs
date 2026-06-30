//! `POST /fees` — add a fee/discount/rebate/amenity to the schedule.

use super::dto::{CreateFeeReq, FeeDto};
use super::{CONDITIONS, KINDS};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::FeeSchedule;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `POST /fees` — create a fee-schedule entry.
#[rocket_okapi::openapi(tag = "Fee Schedule")]
#[post("/fees", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateFeeReq>,
) -> ApiResult<Json<FeeDto>> {
    user.require(Permission::FeeManage)?;
    let b = body.into_inner();
    let code = b.code.trim().to_lowercase();
    if code.is_empty() {
        return Err(ApiError::BadRequest("code is required".into()));
    }
    if !KINDS.contains(&b.kind.as_str()) {
        return Err(ApiError::BadRequest(format!("invalid kind: {}", b.kind)));
    }
    let condition = b.condition_type.unwrap_or_else(|| "manual".into());
    if !CONDITIONS.contains(&condition.as_str()) {
        return Err(ApiError::BadRequest(format!("invalid condition_type: {condition}")));
    }
    if b.amount_cents < 0 {
        return Err(ApiError::BadRequest(
            "amount_cents must be non-negative (kind decides the sign)".into(),
        ));
    }

    if FeeSchedule::find()
        .filter(entity::fee_schedule::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::fee_schedule::Column::Code.eq(code.clone()))
        .one(&state.db)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!("fee code '{code}' already exists")));
    }

    let now = Utc::now();
    let saved = entity::fee_schedule::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        code: Set(code),
        kind: Set(b.kind),
        label: Set(b.label),
        amount_cents: Set(b.amount_cents),
        recurring: Set(b.recurring.unwrap_or(true)),
        condition_type: Set(condition),
        verbiage: Set(b.verbiage),
        active: Set(true),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&state.db)
    .await?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::FEE_SCHEDULE_CREATE,
        Some("fee_schedule"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "code": saved.code, "kind": saved.kind })),
    )
    .await;
    Ok(Json(FeeDto::from(saved)))
}
