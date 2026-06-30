use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct VehicleDto {
    pub id: Uuid,
    pub lease_id: Option<Uuid>,
    pub application_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub make: String,
    pub model: String,
    pub year: Option<i32>,
    pub color: Option<String>,
    pub license_plate: Option<String>,
    pub plate_state: Option<String>,
    pub notes: Option<String>,
    /// One-line human description (used in lease verbiage).
    pub label: String,
}

impl From<entity::vehicle::Model> for VehicleDto {
    fn from(v: entity::vehicle::Model) -> Self {
        let label = crate::leasedoc::describe_vehicle(&v);
        VehicleDto {
            id: v.id,
            lease_id: v.lease_id,
            application_id: v.application_id,
            user_id: v.user_id,
            make: v.make,
            model: v.model,
            year: v.year,
            color: v.color,
            license_plate: v.license_plate,
            plate_state: v.plate_state,
            notes: v.notes,
            label,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateVehicleReq {
    pub lease_id: Option<Uuid>,
    pub application_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub make: String,
    pub model: String,
    pub year: Option<i32>,
    pub color: Option<String>,
    pub license_plate: Option<String>,
    pub plate_state: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateVehicleReq {
    pub lease_id: Option<Uuid>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<i32>,
    pub color: Option<String>,
    pub license_plate: Option<String>,
    pub plate_state: Option<String>,
    pub notes: Option<String>,
}
