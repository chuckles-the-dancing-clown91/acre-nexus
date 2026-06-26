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

/// `GET /properties` — every property in the active tenant's portfolio.
#[rocket_okapi::openapi(tag = "Properties")]
#[get("/properties")]
pub async fn list(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<PropertyResp>>> {
    user.require(Permission::PropertyRead)?;
    let rows = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::property::Column::Name)
        .all(&state.db)
        .await?;
    Ok(Json(rows.into_iter().map(PropertyResp::from).collect()))
}
