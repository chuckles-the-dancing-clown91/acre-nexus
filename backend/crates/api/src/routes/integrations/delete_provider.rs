//! `DELETE /integrations/providers/<id>` — remove a provider + its credential.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::NotificationProvider;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /integrations/providers/<id>` — delete a delivery provider; its
/// vaulted credential is removed with it.
#[rocket_okapi::openapi(tag = "Integrations")]
#[delete("/integrations/providers/<id>")]
pub async fn delete_provider(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::IntegrationsManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    let row = NotificationProvider::find_by_id(id)
        .filter(entity::notification_provider::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("provider not found".into()))?;

    if let Some(key) = &row.secret_ref {
        let _ = crate::secrets::remove(&db, Some(scope.tenant_id), key).await?;
    }
    let channel = row.channel.clone();
    let kind = row.kind.clone();
    row.delete(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::NOTIFICATION_PROVIDER_DELETE,
        Some("notification_provider"),
        Some(id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "channel": channel, "kind": kind })),
    )
    .await;

    Ok(Json(serde_json::json!({ "deleted": true, "id": id })))
}
