use super::dto::ModuleInfo;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::modules::{self, registry};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{get, State};

/// `GET /modules` — every module with its enabled state for this tenant.
#[rocket_okapi::openapi(tag = "Modules")]
#[get("/modules")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    tenant: TenantScope,
) -> ApiResult<Json<Vec<ModuleInfo>>> {
    user.require(Permission::TenantManage)?;

    let mut out = Vec::new();
    for m in registry() {
        let man = m.manifest();
        let enabled = modules::is_enabled(&db, tenant.tenant_id, man.key).await;
        out.push(ModuleInfo {
            key: man.key.to_string(),
            name: man.name.to_string(),
            description: man.description.to_string(),
            permissions: man
                .permissions
                .iter()
                .map(|p| p.as_str().to_string())
                .collect(),
            enabled,
            default_enabled: man.default_enabled,
            preview: man.preview,
        });
    }
    Ok(Json(out))
}
