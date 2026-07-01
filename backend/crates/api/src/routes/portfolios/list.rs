//! `GET /portfolios` — the tenant's portfolios with property counts.

use super::dto::PortfolioResp;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Portfolio, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /portfolios` — list portfolios for the active tenant.
#[rocket_okapi::openapi(tag = "Portfolio")]
#[get("/portfolios")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<PortfolioResp>>> {
    user.require(Permission::PropertyRead)?;
    let portfolios = Portfolio::find()
        .filter(entity::portfolio::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::portfolio::Column::Name)
        .all(&db)
        .await?;
    let props = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?;

    let out = portfolios
        .into_iter()
        .map(|p| {
            let count = props
                .iter()
                .filter(|pr| pr.portfolio_id == Some(p.id))
                .count() as i64;
            PortfolioResp {
                id: p.id,
                name: p.name,
                strategy: p.strategy,
                property_count: count,
            }
        })
        .collect();
    Ok(Json(out))
}
