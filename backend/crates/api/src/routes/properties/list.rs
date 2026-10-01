use super::dto::PropertyResp;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /properties` — the active workspace's properties within the caller's
/// reach (every property for the company; assigned ones for field roles).
#[rocket_okapi::openapi(tag = "Properties")]
#[get("/properties")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    access: crate::tenancy::Access,
) -> ApiResult<Json<Vec<PropertyResp>>> {
    user.require(Permission::PropertyRead)?;
    let mut q = Property::find().filter(entity::property::Column::TenantId.eq(scope.tenant_id));
    if let Some(ids) = access.property_ids() {
        q = q.filter(entity::property::Column::Id.is_in(ids));
    }
    let rows = q
        .order_by_asc(entity::property::Column::Name)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(PropertyResp::from).collect()))
}
