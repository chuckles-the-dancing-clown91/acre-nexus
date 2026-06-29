//! `DELETE /domains/<id>` — remove a white-label domain mapping.

use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Domain;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ColumnTrait, EntityTrait, ModelTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /domains/<id>` — unmap a domain from the active tenant.
#[rocket_okapi::openapi(tag = "Domains")]
#[delete("/domains/<id>")]
pub async fn delete(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::DomainManage)?;
    let did = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid domain id".into()))?;
    let domain = Domain::find()
        .filter(entity::domain::Column::Id.eq(did))
        .filter(entity::domain::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("domain not found".into()))?;
    let hostname = domain.hostname.clone();
    domain.delete(&state.db).await?;

    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::DOMAIN_DELETE,
        Some("domain"),
        Some(did.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "hostname": hostname })),
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
