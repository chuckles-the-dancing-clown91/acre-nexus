//! A **process**: one run of a template against a property (and a unit) — a
//! turnover, an onboarding. Its steps are in [`super::process_step`].

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "process")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `turnover` | `onboarding` | `site_turn`.
    pub kind: String,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub lease_id: Option<Uuid>,
    pub template_id: Option<Uuid>,
    pub title: String,
    /// `active` | `done` | `cancelled`.
    pub status: String,
    /// ISO date the run started (a turn: the move-out).
    pub started_on: String,
    /// ISO date it should be finished by.
    pub target_date: Option<String>,
    pub finished_on: Option<String>,
    /// Why it was finished with required steps still open.
    pub override_reason: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
