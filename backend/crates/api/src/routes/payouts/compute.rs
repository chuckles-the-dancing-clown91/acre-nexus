use super::dto::{ComputePayoutReq, PayoutDto};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::NaiveDate;
use entity::prelude::Llc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// `POST /payouts/compute` — compute a draft payout for an entity + period
/// from settled payments and the expense ledger.
#[rocket_okapi::openapi(tag = "Payouts")]
#[post("/payouts/compute", data = "<body>")]
pub async fn compute_payout(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<ComputePayoutReq>,
) -> ApiResult<Json<PayoutDto>> {
    user.require(Permission::PayoutManage)?;
    let b = body.into_inner();
    let llc = Llc::find_by_id(b.entity_id)
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("legal entity not found".into()))?;

    let start = NaiveDate::parse_from_str(&b.period_start, "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest("period_start must be YYYY-MM-DD".into()))?;
    let end = NaiveDate::parse_from_str(&b.period_end, "%Y-%m-%d")
        .map_err(|_| ApiError::BadRequest("period_end must be YYYY-MM-DD".into()))?;
    if end < start {
        return Err(ApiError::BadRequest(
            "period_end must not precede period_start".into(),
        ));
    }

    let payout = crate::payouts::compute_payout(
        &db,
        scope.tenant_id,
        b.entity_id,
        &b.period_start,
        &b.period_end,
        Some(user.user_id),
    )
    .await?;
    Ok(Json(PayoutDto::from_model(payout, Some(llc.name))))
}
