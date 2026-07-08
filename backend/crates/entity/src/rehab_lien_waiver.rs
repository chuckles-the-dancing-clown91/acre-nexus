//! A **lien waiver** captured for a [`crate::rehab_draw`] — the contractor's
//! release of lien rights in exchange for payment. Four statutory types
//! (conditional/unconditional × progress/final); the generated waiver PDF is
//! filed in the [`crate::document`] service and the signed copy comes back as
//! `received`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "rehab_lien_waiver")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub draw_id: Uuid,
    pub project_id: Uuid,
    /// `conditional_progress` | `unconditional_progress` | `conditional_final` |
    /// `unconditional_final`.
    pub waiver_type: String,
    pub contractor_id: Option<Uuid>,
    /// Denormalised contractor name printed on the waiver.
    pub contractor_name: String,
    pub amount_cents: i64,
    pub through_date: Option<String>,
    /// `generated` | `received`.
    pub status: String,
    /// The generated waiver PDF in the document service.
    pub document_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
