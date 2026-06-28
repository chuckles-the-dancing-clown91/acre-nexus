use super::dto::TokenSummary;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::ApiToken;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /api-tokens` — list the active tenant's API tokens (no secrets).
#[rocket_okapi::openapi(tag = "API Tokens")]
#[get("/api-tokens")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<TokenSummary>>> {
    user.require(Permission::ApiTokenManage)?;
    let rows = ApiToken::find()
        .filter(entity::api_token::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::api_token::Column::CreatedAt)
        .all(&state.user_db)
        .await?;
    Ok(Json(rows.into_iter().map(TokenSummary::from).collect()))
}
