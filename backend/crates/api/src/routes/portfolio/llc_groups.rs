use super::dto::LlcGroup;
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Llc, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

/// `GET /portfolio/llcs` — properties grouped by holding entity.
#[rocket_okapi::openapi(tag = "Portfolio")]
#[get("/portfolio/llcs")]
pub async fn llc_groups(
    _state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
) -> ApiResult<Json<Vec<LlcGroup>>> {
    user.require(Permission::PropertyRead)?;
    let llcs = Llc::find()
        .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
        .order_by_asc(entity::llc::Column::Name)
        .all(&db)
        .await?;
    let props = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?;

    let groups = llcs
        .into_iter()
        .map(|l| {
            let in_llc: Vec<_> = props
                .iter()
                .filter(|p| p.llc_id == Some(l.id))
                .cloned()
                .collect();
            let units: i64 = in_llc.iter().map(|p| p.units as i64).sum();
            let rent: i64 = in_llc.iter().map(|p| p.monthly_rent_cents).sum();
            LlcGroup {
                id: l.id,
                name: l.name,
                ein: l.ein,
                state: l.state,
                property_count: in_llc.len(),
                units,
                monthly_rent_cents: rent,
                monthly_rent_label: usd(rent),
                properties: in_llc
                    .into_iter()
                    .map(crate::routes::properties::PropertyResp::from)
                    .collect(),
            }
        })
        .collect();

    Ok(Json(groups))
}
