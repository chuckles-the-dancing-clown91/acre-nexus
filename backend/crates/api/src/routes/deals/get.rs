use super::dto::{DealDetailDto, DealDto, DealEventDto};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::DealEvent;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /modules/flips/deals/<id>` — a deal with its computed underwriting and
/// event timeline (newest first).
#[rocket_okapi::openapi(tag = "Flips")]
#[get("/modules/flips/deals/<id>")]
pub async fn get(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<DealDetailDto>> {
    user.require(Permission::DealRead)?;
    crate::modules::require_enabled(&state.db, scope.tenant_id, "flips").await?;

    let deal = super::load_deal(&db, scope.tenant_id, id).await?;
    let events = DealEvent::find()
        .filter(entity::deal_event::Column::DealId.eq(deal.id))
        .order_by_desc(entity::deal_event::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(DealDetailDto {
        deal: DealDto::build(&deal),
        events: events.into_iter().map(DealEventDto::from).collect(),
    }))
}
