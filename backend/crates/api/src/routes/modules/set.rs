use super::dto::{ModuleInfo, ToggleModule};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::modules::registry;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::TenantModule;
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /modules/<key>` — enable or disable a module for the active tenant.
/// Upserts the `tenant_module` override row.
#[rocket_okapi::openapi(tag = "Modules")]
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

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::MODULE_TOGGLE,
        Some("module"),
        Some(key.to_string()),
        Some(tenant.tenant_id),
        Some(serde_json::json!({ "enabled": body.enabled })),
    )
    .await;

    Ok(Json(ModuleInfo {
        key: manifest.key.to_string(),
        name: manifest.name.to_string(),
        description: manifest.description.to_string(),
        permissions: manifest
            .permissions
            .iter()
            .map(|p| p.as_str().to_string())
            .collect(),
        enabled: body.enabled,
        default_enabled: manifest.default_enabled,
        preview: manifest.preview,
    }))
}
