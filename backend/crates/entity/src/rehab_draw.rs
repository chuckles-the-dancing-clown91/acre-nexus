//! A **draw request** against a [`crate::rehab_project`] budget — a tranche of
//! money released to a contractor as work completes. Draws move `requested →
//! approved → funded` (or `rejected`); progress photos and supporting documents
//! ride the polymorphic [`crate::document`] service with `owner_type =
//! "rehab_draw"`, and each draw can carry [`crate::rehab_lien_waiver`]s.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rehab_draw")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub project_id: Uuid,
    /// Sequential draw number within the project (1-based).
    pub number: i32,
    pub title: String,
    pub amount_cents: i64,
    /// `requested` | `approved` | `funded` | `rejected`.
    pub status: String,
    /// The contractor paid by this draw (a [`crate::counterparty`]).
    pub contractor_id: Option<Uuid>,
    pub notes: Option<String>,
    pub requested_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub funded_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
