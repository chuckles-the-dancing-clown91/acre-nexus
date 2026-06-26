//! Landlord / property-manager property endpoints (tenant-scoped, RBAC-gated).

use crate::auth::AuthUser;
use crate::dto::usd;
use crate::error::{ApiError, ApiResult};
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use chrono::Utc;
use entity::prelude::Property;
use rocket::serde::json::Json;
use rocket::{get, patch, post, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct PropertyResp {
    pub id: Uuid,
    pub name: String,
    pub address: String,
    pub city: String,
    pub llc_id: Option<Uuid>,
    pub units: i32,
    pub occupied_units: i32,
    pub occupancy: String,
    pub monthly_rent_cents: i64,
    pub monthly_rent_label: String,
    pub status: String,
    pub year_built: i32,
    pub manager: String,
}

impl From<entity::property::Model> for PropertyResp {
    fn from(p: entity::property::Model) -> Self {
        PropertyResp {
            occupancy: format!("{}/{}", p.occupied_units, p.units),
            monthly_rent_label: usd(p.monthly_rent_cents),
            id: p.id,
            name: p.name,
            address: p.address,
            city: p.city,
            llc_id: p.llc_id,
            units: p.units,
            occupied_units: p.occupied_units,
            monthly_rent_cents: p.monthly_rent_cents,
            status: p.status,
            year_built: p.year_built,
            manager: p.manager,
        }
    }
}

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

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreatePropertyReq {
    pub name: String,
    pub address: String,
    pub city: String,
    pub llc_id: Option<Uuid>,
    pub units: i32,
    pub occupied_units: i32,
    pub monthly_rent_cents: i64,
    pub status: Option<String>,
    pub year_built: Option<i32>,
    pub manager: Option<String>,
}

/// `POST /properties` — add a property to the portfolio.
#[rocket_okapi::openapi(tag = "Properties")]
#[post("/properties", data = "<body>")]
pub async fn create(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    body: Json<CreatePropertyReq>,
) -> ApiResult<Json<PropertyResp>> {
    user.require(Permission::PropertyWrite)?;
    let b = body.into_inner();
    let model = entity::property::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(scope.tenant_id),
        llc_id: Set(b.llc_id),
        name: Set(b.name),
        address: Set(b.address),
        city: Set(b.city),
        units: Set(b.units),
        occupied_units: Set(b.occupied_units),
        monthly_rent_cents: Set(b.monthly_rent_cents),
        status: Set(b.status.unwrap_or_else(|| "Stabilized".into())),
        year_built: Set(b.year_built.unwrap_or(0)),
        manager: Set(b.manager.unwrap_or_default()),
        created_at: Set(Utc::now().into()),
    };
    let saved = model.insert(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::PROPERTY_CREATE,
        Some("property"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        Some(serde_json::json!({ "name": saved.name, "city": saved.city })),
    )
    .await;
    Ok(Json(PropertyResp::from(saved)))
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CostLine {
    pub label: String,
    pub amount_cents: i64,
    pub amount_label: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PropertyProfileResp {
    #[serde(flatten)]
    pub property: PropertyResp,
    pub kpis: Vec<CostLine>,
    pub cost_breakdown: Vec<CostLine>,
    pub net_revenue_cents: i64,
    pub net_revenue_label: String,
}

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

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdatePropertyReq {
    pub name: Option<String>,
    pub status: Option<String>,
    pub occupied_units: Option<i32>,
    pub monthly_rent_cents: Option<i64>,
    pub manager: Option<String>,
}

/// `PATCH /properties/<id>` — update mutable property fields.
#[rocket_okapi::openapi(tag = "Properties")]
#[patch("/properties/<id>", data = "<body>")]
pub async fn update(
    state: &State<AppState>,
    user: AuthUser,
    scope: TenantScope,
    id: &str,
    body: Json<UpdatePropertyReq>,
) -> ApiResult<Json<PropertyResp>> {
    user.require(Permission::PropertyWrite)?;
    let pid = Uuid::parse_str(id).map_err(|_| ApiError::BadRequest("invalid id".into()))?;
    let p = Property::find_by_id(pid)
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .one(&state.db)
        .await?
        .ok_or_else(|| ApiError::NotFound("property not found".into()))?;
    let mut am: entity::property::ActiveModel = p.into();
    let b = body.into_inner();
    if let Some(v) = b.name {
        am.name = Set(v);
    }
    if let Some(v) = b.status {
        am.status = Set(v);
    }
    if let Some(v) = b.occupied_units {
        am.occupied_units = Set(v);
    }
    if let Some(v) = b.monthly_rent_cents {
        am.monthly_rent_cents = Set(v);
    }
    if let Some(v) = b.manager {
        am.manager = Set(v);
    }
    let saved = am.update(&state.db).await?;
    crate::audit::record(
        &state.db,
        Some(user.user_id),
        crate::audit::actions::PROPERTY_UPDATE,
        Some("property"),
        Some(saved.id.to_string()),
        Some(scope.tenant_id),
        None,
    )
    .await;
    Ok(Json(PropertyResp::from(saved)))
}
