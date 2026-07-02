//! `PUT /integrations/secrets` — set or rotate a credential.

use super::dto::{SecretDto, SetSecretReq};
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::secrets::{self, StoreOutcome};
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Secret;
use rocket::serde::json::Json;
use rocket::{put, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

/// `PUT /integrations/secrets` — store a credential under a dotted key,
/// creating it or rotating the existing value. The response is the masked
/// listing row; the plaintext is never read back.
#[rocket_okapi::openapi(tag = "Integrations")]
#[put("/integrations/secrets", data = "<body>")]
pub async fn set_secret(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    body: Json<SetSecretReq>,
) -> ApiResult<Json<SecretDto>> {
    user.require(Permission::IntegrationsManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;

    let b = body.into_inner();
    let key = b.key.trim().to_lowercase();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(ApiError::BadRequest(
            "key must be a dotted identifier, e.g. stripe.api_key".into(),
        ));
    }
    if b.value.trim().is_empty() {
        return Err(ApiError::BadRequest("value is required".into()));
    }

    let (_, outcome) = secrets::store(
        &db,
        Some(scope.tenant_id),
        &key,
        b.value.trim(),
        Some(user.user_id),
    )
    .await?;

    // Audit the fact + key name, never the value (docs/AUDIT.md discipline).
    crate::audit::record(
        &db,
        Some(user.user_id),
        match outcome {
            StoreOutcome::Created => crate::audit::actions::SECRET_SET,
            StoreOutcome::Rotated => crate::audit::actions::SECRET_ROTATE,
        },
        Some("secret"),
        Some(key.clone()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "key": key })),
    )
    .await;

    let row = Secret::find()
        .filter(entity::secret::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::secret::Column::Key.eq(key))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("secret disappeared during write".into()))?;
    Ok(Json(SecretDto::from(row)))
}
