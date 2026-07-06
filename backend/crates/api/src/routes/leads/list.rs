use super::dto::{LeadDto, LeadsResp};
use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Lead, Tenant};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

/// `GET /leads?status=` — leasing leads, most recently touched first, plus
/// the monitored inbox address that feeds them.
#[rocket_okapi::openapi(tag = "Leads")]
#[get("/leads?<status>")]
pub async fn list_leads(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    status: Option<&str>,
) -> ApiResult<Json<LeadsResp>> {
    user.require(Permission::ApplicationRead)?;
    let mut q = Lead::find().filter(entity::lead::Column::TenantId.eq(scope.tenant_id));
    if let Some(s) = status.filter(|s| !s.is_empty()) {
        q = q.filter(entity::lead::Column::Status.eq(s));
    }
    let rows = q
        .order_by_desc(entity::lead::Column::UpdatedAt)
        .limit(200)
        .all(&db)
        .await?;
    let inbox_address = Tenant::find_by_id(scope.tenant_id)
        .one(&db)
        .await?
        .map(|t| crate::mail::leasing_address(&t.slug));
    Ok(Json(LeadsResp {
        inbox_address,
        leads: rows.into_iter().map(LeadDto::from).collect(),
    }))
}
