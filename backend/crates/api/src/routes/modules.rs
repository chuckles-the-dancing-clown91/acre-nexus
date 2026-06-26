//! Tenant-facing module management: list available modules with their enabled
//! state, and toggle a module on/off. These power the "Modules" section of a
//! tenant's software settings. Gated by `tenant:manage`.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::modules::{self, registry};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::TenantModule;
use rocket::serde::json::Json;
use rocket::{get, patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A module plus its resolved enablement for the active tenant.
#[derive(Serialize)]
pub struct ModuleInfo {
    pub key: String,
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub enabled: bool,
    pub default_enabled: bool,
    pub preview: bool,
}

/// `GET /modules` — every module with its enabled state for this tenant.
#[get("/modules")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    tenant: TenantScope,
) -> ApiResult<Json<Vec<ModuleInfo>>> {
    user.require(Permission::TenantManage)?;

    let mut out = Vec::new();
    for m in registry() {
        let man = m.manifest();
        let enabled = modules::is_enabled(&state.db, tenant.tenant_id, man.key).await;
        out.push(ModuleInfo {
            key: man.key.to_string(),
            name: man.name.to_string(),
            description: man.description.to_string(),
            permissions: man.permissions.iter().map(|p| p.as_str().to_string()).collect(),
            enabled,
            default_enabled: man.default_enabled,
            preview: man.preview,
        });
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
pub struct ToggleModule {
    pub enabled: bool,
}

/// `PATCH /modules/<key>` — enable or disable a module for the active tenant.
/// Upserts the `tenant_module` override row.
#[patch("/modules/<key>", data = "<body>")]
pub async fn set(
    state: &State<AppState>,
    user: AuthUser,
    tenant: TenantScope,
    key: &str,
    body: Json<ToggleModule>,
) -> ApiResult<Json<ModuleInfo>> {
    user.require(Permission::TenantManage)?;

    // Reject unknown module keys so the settings UI can't drift from the backend.
    let manifest = registry()
        .into_iter()
        .map(|m| m.manifest())
        .find(|m| m.key == key)
        .ok_or_else(|| ApiError::NotFound(format!("unknown module: {key}")))?;

    let existing = TenantModule::find()
        .filter(entity::tenant_module::Column::TenantId.eq(tenant.tenant_id))
        .filter(entity::tenant_module::Column::ModuleKey.eq(key))
        .one(&state.db)
        .await?;

    match existing {
        Some(row) => {
            let mut am: entity::tenant_module::ActiveModel = row.into();
            am.enabled = Set(body.enabled);
            am.updated_at = Set(Utc::now().into());
            am.update(&state.db).await?;
        }
        None => {
            entity::tenant_module::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant.tenant_id),
                module_key: Set(key.to_string()),
                enabled: Set(body.enabled),
                updated_at: Set(Utc::now().into()),
            }
            .insert(&state.db)
            .await?;
        }
    }

    Ok(Json(ModuleInfo {
        key: manifest.key.to_string(),
        name: manifest.name.to_string(),
        description: manifest.description.to_string(),
        permissions: manifest.permissions.iter().map(|p| p.as_str().to_string()).collect(),
        enabled: body.enabled,
        default_enabled: manifest.default_enabled,
        preview: manifest.preview,
    }))
}
