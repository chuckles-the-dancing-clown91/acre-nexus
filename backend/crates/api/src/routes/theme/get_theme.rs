use super::dto::ThemeResp;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Theme;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// `GET /theme` — the active tenant's theme configuration.
#[rocket_okapi::openapi(tag = "Theming")]
#[get("/theme")]
pub async fn get_theme(
    state: &State<AppState>,
    _user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<ThemeResp>> {
    let t = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("theme not configured".into()))?;
    Ok(Json(ThemeResp::from(t)))
}
