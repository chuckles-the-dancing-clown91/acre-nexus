//! An immutable **application workflow transition**: one status change on a
//! rental [`crate::application`] as it moves through the pipeline
//! (New → Screening → Approved → Leased, with Declined / Withdrawn off-ramps).
//! Mirrors [`crate::workflow_event`] for properties, giving the applications
//! pipeline an auditable, resumable history.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "application_event")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub application_id: Uuid,
    /// `NULL` for the initial state (submission).
    pub from_status: Option<String>,
    pub to_status: String,
    pub note: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
