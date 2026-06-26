use super::dto::{CostLine, PropertyProfileResp, PropertyResp};
use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

/// `GET /properties/<id>` — full property profile with computed economics.
///
/// Economics mirror the design prototype: maintenance ≈ 9% of rent, taxes &
/// insurance ≈ 12%, management fee 8%; net = rent − those.
#[rocket_okapi::openapi(tag = "Properties")]
#[get("/properties/<id>")]
pub async fn profile(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
) -> ApiResult<Json<PropertyProfileResp>> {
    user.require(Permission::PropertyRead)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let p = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;

    let rent = p.monthly_rent_cents;
    let maint = (rent as f64 * 0.09).round() as i64;
    let tax = (rent as f64 * 0.12).round() as i64;
    let mgmt = (rent as f64 * 0.08).round() as i64;
    let net = rent - maint - tax - mgmt;

    let line = |label: &str, cents: i64| CostLine {
        label: label.into(),
        amount_cents: cents,
        amount_label: usd(cents),
    };

    let occupancy = format!("{}/{}", p.occupied_units, p.units);
    let kpis = vec![
        line("Monthly rent", rent),
        CostLine {
            label: "Occupancy".into(),
            amount_cents: p.occupied_units as i64,
            amount_label: occupancy.clone(),
        },
        line("Maintenance MTD", maint),
        line("Net revenue", net),
    ];
    let cost_breakdown = vec![
        line("Rent income", rent),
        line("Maintenance & repairs", -maint),
        line("Taxes & insurance", -tax),
        line("Management fee (8%)", -mgmt),
    ];

    Ok(Json(PropertyProfileResp {
        property: PropertyResp::from(p),
        kpis,
        cost_breakdown,
        net_revenue_cents: net,
        net_revenue_label: usd(net),
    }))
}
