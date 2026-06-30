//! `POST /portfolios` — create a property grouping.

use super::dto::{CreatePortfolioReq, PortfolioResp};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

/// `POST /portfolios` — create a portfolio.
#[rocket_okapi::openapi(tag = "Portfolio")]
#[post("/portfolios", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreatePortfolioReq>,
) -> ApiResult<Json<PortfolioResp>> {
    user.require(Permission::PropertyWrite)?;
    let b = body.into_inner();
    if b.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name is required".into()));
    }
    let id = Uuid::new_v4();
    let saved = entity::portfolio::ActiveModel {
        id: Set(id),
        tenant_id: Set(scope.tenant_id),
        name: Set(b.name.trim().to_string()),
        strategy: Set(b.strategy.unwrap_or_default()),
        created_at: Set(Utc::now().into()),
    }
    .insert(&state.db)
    .await?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::PORTFOLIO_CREATE,
        Some("portfolio"),
        Some(id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "name": saved.name })),
    )
    .await;

    Ok(Json(PortfolioResp {
        id: saved.id,
        name: saved.name,
        strategy: saved.strategy,
        property_count: 0,
    }))
}
