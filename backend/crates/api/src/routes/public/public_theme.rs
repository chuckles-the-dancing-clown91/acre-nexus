use super::dto::PublicTheme;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use crate::tenancy::PublicTenant;
use entity::prelude::Theme;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// `GET /public/theme` — branding for the resolved tenant.
#[rocket_okapi::openapi(tag = "Public Website")]
#[get("/public/theme")]
pub async fn public_theme(
    state: &State<AppState>,
    tenant: PublicTenant,
) -> ApiResult<Json<PublicTheme>> {
    let t = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant.tenant_id))
        .one(&state.user_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("theme not configured".into()))?;
    Ok(Json(PublicTheme {
        company_name: t.company_name,
        logo_url: t.logo_url,
        primary_color: t.primary_color,
        accent_color: t.accent_color,
        default_mode: t.default_mode,
    }))
}
