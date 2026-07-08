//! A timestamped event in a [`crate::deal`]'s history — the acquisition
//! equivalent of [`crate::workflow_event`]. Records stage transitions, offers,
//! notes, and the final conversion into an owned property, so the deal detail
//! view can render a full timeline.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "deal_event")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub deal_id: Uuid,
    /// `created` | `stage_change` | `offer` | `note` | `converted`.
    pub kind: String,
    pub from_stage: Option<String>,
    pub to_stage: Option<String>,
    pub body: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
