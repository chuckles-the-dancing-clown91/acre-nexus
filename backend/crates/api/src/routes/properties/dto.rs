use crate::dto::usd;
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

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdatePropertyReq {
    pub name: Option<String>,
    pub status: Option<String>,
    pub occupied_units: Option<i32>,
    pub monthly_rent_cents: Option<i64>,
    pub manager: Option<String>,
}
