//! `GET /domains` — the white-label domains mapped to the active tenant.

use super::dto::DomainResp;
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Domain;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /domains` — list this tenant's domains (admin / owner / renter).
#[rocket_okapi::openapi(tag = "Domains")]
#[get("/domains")]
pub async fn list(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<DomainResp>>> {
    user.require(Permission::DomainRead)?;
    let rows = Domain::find()
        .filter(entity::domain::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::domain::Column::Hostname)
        .all(&db)
        .await?;
    Ok(Json(rows.into_iter().map(DomainResp::from).collect()))
}
