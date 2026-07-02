//! `PATCH /integrations/providers/<id>` — edit / rotate / toggle a provider.

use super::create_provider::clear_default;
use super::dto::{ProviderDto, UpdateProviderReq};
use super::provider_secret_ref;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::{NotificationProvider, Secret};
use rocket::serde::json::Json;
use rocket::{patch, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `PATCH /integrations/providers/<id>` — update config, rotate the vaulted
/// credential, enable/disable, or promote to the channel default.
#[rocket_okapi::openapi(tag = "Integrations")]
#[patch("/integrations/providers/<id>", data = "<body>")]
pub async fn update_provider(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdateProviderReq>,
) -> ApiResult<Json<ProviderDto>> {
    user.require(Permission::IntegrationsManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    let row = NotificationProvider::find_by_id(id)
        .filter(entity::notification_provider::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("provider not found".into()))?;
    let channel = row.channel.clone();
    let kind = row.kind.clone();
    let existing_secret_ref = row.secret_ref.clone();

    let b = body.into_inner();
    let mut changed: Vec<&str> = Vec::new();
    let mut am: entity::notification_provider::ActiveModel = row.into();

    if let Some(config) = b.config {
        if !config.is_object() {
            return Err(ApiError::BadRequest("config must be a JSON object".into()));
        }
        am.config = Set(config);
        changed.push("config");
    }
    if let Some(cred) = b.credential.as_deref().map(str::trim) {
        if cred.is_empty() {
            return Err(ApiError::BadRequest("credential cannot be empty".into()));
        }
        let key = existing_secret_ref.unwrap_or_else(|| provider_secret_ref(id));
        crate::secrets::store(&db, Some(scope.tenant_id), &key, cred, Some(user.user_id)).await?;
        am.secret_ref = Set(Some(key));
        changed.push("credential");
    }
    if let Some(enabled) = b.enabled {
        am.enabled = Set(enabled);
        changed.push("enabled");
    }
    if let Some(is_default) = b.is_default {
        if is_default {
            clear_default(&db, scope.tenant_id, &channel).await?;
        }
        am.is_default = Set(is_default);
        changed.push("is_default");
    }
    am.updated_at = Set(Utc::now().into());
    let saved = am.update(&db).await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::NOTIFICATION_PROVIDER_UPDATE,
        Some("notification_provider"),
        Some(id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "channel": channel, "kind": kind, "fields": changed })),
    )
    .await;

    let last4 = match &saved.secret_ref {
        Some(key) => Secret::find()
            .filter(entity::secret::Column::TenantId.eq(scope.tenant_id))
            .filter(entity::secret::Column::Key.eq(key.clone()))
            .one(&db)
            .await?
            .map(|s| s.last4),
        None => None,
    };
    Ok(Json(ProviderDto::from_model(saved, last4)))
}
