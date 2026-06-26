//! Per-tenant theming / white-label configuration (authenticated side).

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Theme;
use rocket::serde::json::Json;
use rocket::{get, put, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

#[derive(Serialize, schemars::JsonSchema)]
pub struct ThemeResp {
    pub company_name: String,
    pub logo_url: Option<String>,
    pub primary_color: String,
    pub accent_color: String,
    pub default_mode: String,
    pub legal_templates: serde_json::Value,
}

impl From<entity::theme::Model> for ThemeResp {
    fn from(t: entity::theme::Model) -> Self {
        ThemeResp {
            company_name: t.company_name,
            logo_url: t.logo_url,
            primary_color: t.primary_color,
            accent_color: t.accent_color,
            default_mode: t.default_mode,
            legal_templates: t.legal_templates,
        }
    }
}

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

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateThemeReq {
    pub company_name: Option<String>,
    pub logo_url: Option<String>,
    pub primary_color: Option<String>,
    pub accent_color: Option<String>,
    pub default_mode: Option<String>,
    pub legal_templates: Option<serde_json::Value>,
}

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
        .one(&state.db)
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
    let saved = am.update(&state.db).await?;
    Ok(Json(ThemeResp::from(saved)))
}
