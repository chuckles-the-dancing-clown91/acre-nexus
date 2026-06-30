//! `GET /properties/<id>/tenant-history` — the turnover timeline for one property:
//! every resident who has leased there, current and former.

use super::build_history;
use super::dto::TenantHistoryRow;
use crate::auth::AuthUser;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;
use uuid::Uuid;

/// `GET /properties/<id>/tenant-history` — resident history for one property.
#[rocket_okapi::openapi(tag = "Tenant History")]
#[get("/properties/<id>/tenant-history")]
pub async fn property_history(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<Vec<TenantHistoryRow>>> {
    user.require(Permission::LeaseRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let prop = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .filter(entity::lease::Column::PropertyId.eq(pid))
        .all(&state.db)
        .await?;
    let mut prop_names = HashMap::new();
    prop_names.insert(prop.id, prop.name);
    Ok(Json(build_history(leases, &prop_names)))
}
