use super::dto::PayoutDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::OwnerPayout;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

/// `GET /payouts` — owner payouts, newest first.
#[rocket_okapi::openapi(tag = "Payouts")]
#[get("/payouts")]
pub async fn list_payouts(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<PayoutDto>>> {
    user.require(Permission::LedgerRead)?;
    let names = crate::payouts::entity_names(&db, scope.tenant_id).await?;
    let rows = OwnerPayout::find()
        .filter(entity::owner_payout::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::owner_payout::Column::CreatedAt)
        .limit(100)
        .all(&db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|p| {
                let name = names.get(&p.entity_id).cloned();
                PayoutDto::from_model(p, name)
            })
            .collect(),
    ))
}
