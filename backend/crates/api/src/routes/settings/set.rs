//! `PUT /settings/<key>` — override one setting for the active tenant.

use super::{SetSettingReq, SettingView};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::settings;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{put, State};

/// `PUT /settings/<key>` — set a setting's value (validated against the catalog).
#[rocket_okapi::openapi(tag = "Settings")]
#[put("/settings/<key>", data = "<body>")]
pub async fn set(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    key: &str,
    body: Json<SetSettingReq>,
) -> ApiResult<Json<SettingView>> {
    user.require(Permission::TenantManage)?;
    let d = settings::def(key)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown setting: {key}")))?;
    let value = settings::set_value(&db, scope.tenant_id, key, body.into_inner().value).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::SETTING_UPDATE,
        Some("setting"),
        Some(key.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "key": key, "value": value })),
    )
    .await;

    Ok(Json(SettingView {
        key: d.key.to_string(),
        label: d.label.to_string(),
        description: d.description.to_string(),
        group: d.group.to_string(),
        kind: d.kind.as_str().to_string(),
        value,
        default: (d.default)(),
    }))
}
