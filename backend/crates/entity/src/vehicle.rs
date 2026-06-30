//! A **vehicle** profile — a resident's car/truck, captured during application or
//! lease onboarding. Garage and parking amenities pull these details into the
//! generated lease document. Optionally linked to an `application`, a `lease`,
//! and/or an `app_user` (renter), all nullable so a vehicle can be attached at any
//! stage of the lifecycle.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "vehicle")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Option<Uuid>,
    pub application_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub make: String,
    pub model: String,
    pub year: Option<i32>,
    pub color: Option<String>,
    pub license_plate: Option<String>,
    /// Two-letter plate-issuing state.
    pub plate_state: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
