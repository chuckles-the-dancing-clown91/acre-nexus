//! A resident's landlord-facing profile: work, emergency contact, household,
//! pets and rental history. One row per person, in their company's workspace.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "resident_profile")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub employer: Option<String>,
    pub job_title: Option<String>,
    pub employer_phone: Option<String>,
    pub emergency_contact_name: Option<String>,
    pub emergency_contact_phone: Option<String>,
    pub emergency_contact_relation: Option<String>,
    /// `[{ name, relation, age }]`
    pub occupants: Json,
    /// `[{ name, kind, breed, weight_lb, color, age_years, service_animal, vaccinated_through, notes }]`
    pub pets: Json,
    /// `[{ address, landlord_name, landlord_phone, rent_cents, from, to, reason_for_leaving }]`
    pub prior_rentals: Json,
    /// Seen by staff only.
    pub staff_notes: Option<String>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
