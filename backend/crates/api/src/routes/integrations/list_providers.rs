//! `GET /integrations/providers` — the tenant's delivery providers, masked.

use super::dto::ProviderDto;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{NotificationProvider, Secret};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /integrations/providers` — list configured notification delivery
/// providers (credentials shown as `last4` only).
#[rocket_okapi::openapi(tag = "Integrations")]
#[get("/integrations/providers")]
pub async fn list_providers(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<ProviderDto>>> {
    user.require(Permission::IntegrationsManage)?;
    crate::modules::require_enabled(&db, scope.tenant_id, "integrations").await?;

    let rows = NotificationProvider::find()
        .filter(entity::notification_provider::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::notification_provider::Column::Channel)
        .order_by_asc(entity::notification_provider::Column::CreatedAt)
        .all(&db)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let last4 = match &row.secret_ref {
            Some(key) => Secret::find()
                .filter(entity::secret::Column::TenantId.eq(scope.tenant_id))
                .filter(entity::secret::Column::Key.eq(key.clone()))
                .one(&db)
                .await?
                .map(|s| s.last4),
            None => None,
        };
        out.push(ProviderDto::from_model(row, last4));
    }
    Ok(Json(out))
}
