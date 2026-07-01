use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::ApiToken;
use rocket::serde::json::Json;
use rocket::{delete, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

/// `DELETE /api-tokens/<id>` — revoke a token immediately.
#[rocket_okapi::openapi(tag = "API Tokens")]
#[delete("/api-tokens/<id>")]
pub async fn revoke(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<serde_json::Value>> {
    user.require(Permission::ApiTokenManage)?;
    let tid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let t = ApiToken::find_by_id(tid)
        .filter(entity::api_token::Column::TenantId.eq(scope.tenant_id))
        .one(&db)
        .await?
        .ok_or_else(|| ApiError::NotFound("token not found".into()))?;
    let mut am: entity::api_token::ActiveModel = t.into();
    am.revoked_at = Set(Some(Utc::now().into()));
    am.update(&db).await?;
    crate::audit::record(
        &db,
        Some(user.user_id),
        crate::audit::actions::TOKEN_REVOKE,
        Some("api_token"),
        Some(tid.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(serde_json::json!({ "ok": true })))
}
