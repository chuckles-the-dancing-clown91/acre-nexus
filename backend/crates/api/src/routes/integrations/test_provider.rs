//! `POST /integrations/providers/<id>/test` — send a test notification
//! through one specific provider.

use super::dto::TestProviderReq;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{NotificationProvider, User};
use rocket::serde::json::Json;
use rocket::{post, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `POST /integrations/providers/<id>/test` — enqueue a `test_notification`
/// send routed through this provider (bypassing the channel default). Email
/// tests default to your account email; SMS tests need a `to` phone number.
#[rocket_okapi::openapi(tag = "Integrations")]
#[post("/integrations/providers/<id>/test", data = "<body>")]
pub async fn test_provider(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<TestProviderReq>,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::IntegrationsManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;
    let id = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;

    let provider = NotificationProvider::find_by_id(id)
        .filter(entity::notification_provider::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("provider not found".into()))?;

    let me = User::find_by_id(user.user_id)
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("user not found".into()))?;

    let to = body.into_inner().to.map(|t| t.trim().to_string());
    let (kind, to) = match provider.channel.as_str() {
        "email" => ("auto_email", to.unwrap_or(me.email)),
        "sms" => match to {
            Some(t) if !t.is_empty() => ("auto_sms", t),
            _ => {
                return Err(ApiError::BadRequest(
                    "SMS tests need a 'to' phone number".into(),
                ))
            }
        },
        "chat" => ("auto_chat", provider.kind.clone()),
        other => {
            return Err(ApiError::BadRequest(format!(
                "channel {other} is not testable"
            )))
        }
    };

    // provider_id routes the send through THIS provider; no owner fields, so
    // repeat tests never dedupe away.
    let job_id = crate::scheduler::enqueue(
        &db,
        scope.tenant_id,
        kind,
        serde_json::json!({
            "template": "test_notification",
            "to": to,
            "provider_id": provider.id.to_string(),
        }),
        0,
    )
    .await?;

    // The recipient is logged, never the credentials the provider carries.
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::NOTIFICATION_TEST,
        Some("notification_provider"),
        Some(provider.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "channel": provider.channel, "to": to, "job_id": job_id })),
    )
    .await;

    Ok(Json(
        serde_json::json!({ "queued": true, "job_id": job_id }),
    ))
}
