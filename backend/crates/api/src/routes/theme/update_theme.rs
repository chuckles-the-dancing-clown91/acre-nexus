use super::dto::{ThemeResp, UpdateThemeReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Theme;
use rocket::serde::json::Json;
use rocket::{put, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

/// `PUT /theme` — update branding, colours and legal boilerplate templates.
#[rocket_okapi::openapi(tag = "Theming")]
#[put("/theme", data = "<body>")]
pub async fn update_theme(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<UpdateThemeReq>,
) -> ApiResult<Json<ThemeResp>> {
    user.require(Permission::ThemeWrite)?;
    let t = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(scope.tenant_id))
        .one(&state.user_db)
        .await?
        .ok_or_else(|| ApiError::NotFound("theme not configured".into()))?;
    let mut am: entity::theme::ActiveModel = t.into();
    let b = body.into_inner();
    if let Some(v) = b.company_name {
        am.company_name = Set(v);
    }
    if let Some(v) = b.logo_url {
        am.logo_url = Set(Some(v));
    }
    if let Some(v) = b.primary_color {
        am.primary_color = Set(v);
    }
    if let Some(v) = b.accent_color {
        am.accent_color = Set(v);
    }
    if let Some(v) = b.default_mode {
        am.default_mode = Set(v);
    }
    if let Some(v) = b.legal_templates {
        am.legal_templates = Set(v);
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&state.user_db).await?;
    crate::audit::record(
        &state.user_db,
        Some(user.user_id),
        crate::audit::actions::THEME_UPDATE,
        Some("theme"),
        None,
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(ThemeResp::from(saved)))
}
