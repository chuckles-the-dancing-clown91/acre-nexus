//! `GET /integrations/secrets` — the tenant's stored credentials, masked.

use super::dto::SecretDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Secret;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /integrations/secrets` — list this workspace's integration credentials
/// (key + `last4` only; plaintext is never returned by any endpoint).
#[rocket_okapi::openapi(tag = "Integrations")]
#[get("/integrations/secrets")]
pub async fn list_secrets(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<SecretDto>>> {
    user.require(Permission::IntegrationsManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;

    let rows = Secret::find()
        .filter(entity::secret::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::secret::Column::Key)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(SecretDto::from).collect()))
}
