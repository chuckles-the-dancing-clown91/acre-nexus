use super::dto::PayoutDto;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::OwnerPayout;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `POST /payouts/<id>/execute` — execute a draft payout as an ACH transfer
/// (sandbox by default). Settlement posts the ledger entry + statement.
#[rocket_okapi::openapi(tag = "Payouts")]
#[post("/payouts/<id>/execute")]
pub async fn execute_payout(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PayoutDto>> {
    user.require(Permission::PayoutManage)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let payout = OwnerPayout::find_by_id(pid)
        .filter(entity::owner_payout::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("payout not found".into()))?;
    let names = crate::payouts::entity_names(&db, scope.tenant_id).await?;
    let name = names.get(&payout.entity_id).cloned();
    let saved = crate::payouts::execute_payout(&db, scope.tenant_id, payout, user.user_id).await?;
    Ok(Json(PayoutDto::from_model(saved, name)))
}
