//! `POST /integrations/providers` — configure a delivery provider.

use super::dto::{CreateProviderReq, ProviderDto};
use super::provider_secret_ref;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::NotificationProvider;
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// Validate a (channel, kind) pair against the provider catalog.
pub fn validate_channel_kind(channel: &str, kind: &str) -> Result<(), ApiError> {
    let kinds = crate::notify::PROVIDER_CHANNELS
        .iter()
        .find(|(c, _)| *c == channel)
        .map(|(_, k)| *k)
        .ok_or_else(|| {
            ApiError::BadRequest(format!(
                "invalid channel: {channel} (expected one of {})",
                crate::notify::PROVIDER_CHANNELS
                    .iter()
                    .map(|(c, _)| *c)
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
    if !kinds.contains(&kind) {
        return Err(ApiError::BadRequest(format!(
            "invalid kind for {channel}: {kind} (expected one of {})",
            kinds.join(", ")
        )));
    }
    Ok(())
}

/// Clear the current default for a channel (before promoting another row).
pub async fn clear_default(
    db: &impl sea_orm::ConnectionTrait,
    tenant_id: Uuid,
    channel: &str,
) -> Result<(), sea_orm::DbErr> {
    NotificationProvider::update_many()
        .col_expr(
            entity::notification_provider::Column::IsDefault,
            sea_orm::sea_query::Expr::value(false),
        )
        .filter(entity::notification_provider::Column::TenantId.eq(tenant_id))
        .filter(entity::notification_provider::Column::Channel.eq(channel))
        .filter(entity::notification_provider::Column::IsDefault.eq(true))
        .exec(db)
        .await?;
    Ok(())
}

/// `POST /integrations/providers` — add a notification delivery provider. The
/// credential (API key / auth token / webhook URL) goes straight to the
/// secrets vault; the response only ever carries its `last4`.
#[rocket_okapi::openapi(tag = "Integrations")]
#[post("/integrations/providers", data = "<body>")]
pub async fn create_provider(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreateProviderReq>,
) -> ApiResult<Json<ProviderDto>> {
    user.require(Permission::IntegrationsManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;

    let b = body.into_inner();
    let channel = b.channel.trim().to_lowercase();
    let kind = b.kind.trim().to_lowercase();
    validate_channel_kind(&channel, &kind)?;
    let config = b.config.unwrap_or_else(|| serde_json::json!({}));
    if !config.is_object() {
        return Err(ApiError::BadRequest("config must be a JSON object".into()));
    }

    let id = Uuid::new_v4();
    let secret_ref = match b.credential.as_deref().map(str::trim) {
        Some(cred) if !cred.is_empty() => {
            let key = provider_secret_ref(id);
            crate::secrets::store(&db, Some(scope.tenant_id), &key, cred, Some(user.user_id))
                .await?;
            Some(key)
        }
        _ => None,
    };

    // First provider for a channel becomes the default automatically.
    let has_default = crate::notify::default_provider(&db, scope.tenant_id, &channel)
        .await
        .map(|p| p.is_default)
        .unwrap_or(false);
    let make_default = b.is_default.unwrap_or(!has_default);
    if make_default {
        clear_default(&db, scope.tenant_id, &channel).await?;
    }

    let now = Utc::now();
    let saved = entity::notification_provider::ActiveModel {
        id: Set(id),
        tenant_id: Set(scope.tenant_id),
        channel: Set(channel.clone()),
        kind: Set(kind.clone()),
        config: Set(config),
        secret_ref: Set(secret_ref),
        enabled: Set(true),
        is_default: Set(make_default),
        created_by: Set(Some(user.user_id)),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await?;

    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::NOTIFICATION_PROVIDER_CREATE,
        Some("notification_provider"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "channel": channel, "kind": kind })),
    )
    .await;

    let last4 = b
        .credential
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(crate::pii::last4);
    Ok(Json(ProviderDto::from_model(saved, last4)))
}
