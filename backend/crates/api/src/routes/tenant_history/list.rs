//! `GET /tenant-history` — every resident the workspace has leased to, past and
//! present, with their tenancy timeline and current standing.

use super::build_history;
use super::dto::TenantHistoryRow;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lease, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;

/// `GET /tenant-history` — resident history across the whole workspace.
#[rocket_okapi::openapi(tag = "Tenant History")]
#[get("/tenant-history")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<TenantHistoryRow>>> {
    user.require(Permission::LeaseRead)?;
    let leases = Lease::find()
        .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?;
    let prop_names: HashMap<_, _> = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect();
    Ok(Json(build_history(leases, &prop_names)))
}
