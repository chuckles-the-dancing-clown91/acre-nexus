//! `GET /settings` — the tenant's full settings catalog with effective values.

use super::SettingView;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::settings;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{get, State};

/// `GET /settings` — list every setting with the tenant's effective value.
#[rocket_okapi::openapi(tag = "Settings")]
#[get("/settings")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<SettingView>>> {
    user.require(Permission::TenantManage)?;
    let mut out = Vec::with_capacity(settings::CATALOG.len());
    for d in settings::CATALOG {
        out.push(SettingView {
            key: d.key.to_string(),
            label: d.label.to_string(),
            description: d.description.to_string(),
            group: d.group.to_string(),
            kind: d.kind.as_str().to_string(),
            value: settings::get_value(&db, scope.tenant_id, d.key).await,
            default: (d.default)(),
        });
    }
    Ok(Json(out))
}
