use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct VendorListing {
    pub id: Uuid,
    pub title: String,
    pub city: String,
    pub beds: i32,
    pub baths: i32,
    pub rent: String,
    pub status: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct VendorProperty {
    pub id: Uuid,
    pub name: String,
    pub city: String,
    pub units: i32,
    pub occupancy: String,
    pub monthly_rent: String,
    pub status: String,
}
