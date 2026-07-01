use super::dto::ApplicationResp;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Application;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /applications` — applications for the active tenant.
#[rocket_okapi::openapi(tag = "Applications")]
#[get("/applications")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<ApplicationResp>>> {
    user.require(Permission::ApplicationRead)?;
    let rows = Application::find()
        .filter(entity::application::Column::TenantId.eq(scope.tenant_id))
        .order_by_desc(entity::application::Column::CreatedAt)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(ApplicationResp::from).collect()))
}
