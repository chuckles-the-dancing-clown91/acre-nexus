use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Mortgage;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /mortgages/<id>` — remove a mortgage from a property.
#[rocket_okapi::openapi(tag = "Financing")]
#[delete("/mortgages/<id>")]
pub async fn delete(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::FinanceManage)?;
    let mid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Mortgage::find_by_id(mid)
        .filter(entity::mortgage::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("mortgage not found".into()))?;
    Mortgage::delete_by_id(mid).exec(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::MORTGAGE_DELETE,
        Some("mortgage"),
        Some(mid.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
