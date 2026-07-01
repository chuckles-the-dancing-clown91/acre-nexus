use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Ownership;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `DELETE /ownership/<id>` — remove an ownership record.
#[rocket_okapi::openapi(tag = "Title")]
#[delete("/ownership/<id>")]
pub async fn delete_ownership(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::TitleManage)?;
    let oid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    Ownership::find_by_id(oid)
        .filter(entity::ownership::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("ownership not found".into()))?;
    Ownership::delete_by_id(oid).exec(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::OWNERSHIP_DELETE,
        Some("ownership"),
        Some(oid.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "deleted": true })))
}
