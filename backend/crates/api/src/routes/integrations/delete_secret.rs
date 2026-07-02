//! `DELETE /integrations/secrets/<key>` — remove a credential.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::secrets;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use rocket::serde::json::Json;
use rocket::{delete, State};

/// `DELETE /integrations/secrets/<key>` — delete a stored credential.
#[rocket_okapi::openapi(tag = "Integrations")]
#[delete("/integrations/secrets/<key>")]
pub async fn delete_secret(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    key: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::IntegrationsManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;

    let key = key.trim().to_lowercase();
    let removed = secrets::remove(&db, Some(scope.tenant_id), &key).await?;
    if !removed {
        return Err(ApiError::NotFound(format!(
            "no secret stored under '{key}'"
        )));
    }

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::SECRET_DELETE,
        Some("secret"),
        Some(key.clone()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "key": key })),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true, "key": key })))
}
