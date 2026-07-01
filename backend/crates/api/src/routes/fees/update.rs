//! `PATCH /fees/<id>` — edit a fee-schedule entry.

use super::dto::{FeeDto, UpdateFeeReq};
use super::CONDITIONS;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::FeeSchedule;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /fees/<id>` — update a fee-schedule entry.
#[rocket_okapi::openapi(tag = "Fee Schedule")]
#[patch("/fees/<id>", data = "<body>")]
pub async fn update(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateFeeReq>,
) -> ApiResult<Json<FeeDto>> {
    user.require(Permission::FeeManage)?;
    let fid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let existing = FeeSchedule::find_by_id(fid)
        .filter(entity::fee_schedule::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("fee not found".into()))?;
    let b = body.into_inner();
    if let Some(c) = &b.condition_type {
        if !CONDITIONS.contains(&c.as_str()) {
            return Err(ApiError::BadRequest(format!("invalid condition_type: {c}")));
        }
    }
    let mut am: entity::fee_schedule::ActiveModel = existing.into();
    if let Some(v) = b.label {
        am.label = Set(v);
    }
    if let Some(v) = b.amount_cents {
        if v < 0 {
            return Err(ApiError::BadRequest(
                "amount_cents must be non-negative".into(),
            ));
        }
        am.amount_cents = Set(v);
    }
    if let Some(v) = b.recurring {
        am.recurring = Set(v);
    }
    if let Some(v) = b.condition_type {
        am.condition_type = Set(v);
    }
    if let Some(v) = b.verbiage {
        am.verbiage = Set(Some(v));
    }
    if let Some(v) = b.active {
        am.active = Set(v);
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::FEE_SCHEDULE_UPDATE,
        Some("fee_schedule"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(FeeDto::from(saved)))
}
