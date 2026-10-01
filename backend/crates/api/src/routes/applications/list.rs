use super::dto::ApplicationResp;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Application;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

/// `GET /applications?limit&before` — applications for the active tenant,
/// newest first: at most `limit` (default 200, max 500), older than the
/// `before` cursor (the `created_at` of the last one seen) when given.
#[rocket_okapi::openapi(tag = "Applications")]
#[get("/applications?<limit>&<before>")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    limit: Option<u64>,
    before: Option<String>,
) -> ApiResult<Json<Vec<ApplicationResp>>> {
    user.require(Permission::ApplicationRead)?;
    let mut q =
        Application::find().filter(entity::application::Column::TenantId.eq(scope.tenant_id));
    if let Some(b) = crate::paging::before(before.as_deref())? {
        q = q.filter(entity::application::Column::CreatedAt.lt(b));
    }
    let rows = q
        .order_by_desc(entity::application::Column::CreatedAt)
        .limit(crate::paging::limit(limit, 200, 500))
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(ApplicationResp::from).collect()))
}
