//! A **workflow event** records a stage transition in a property's investment
//! workflow (flip / rental / BRRRR / hold). Together these form the timestamped
//! history behind a property's current `workflow_stage`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "workflow_event")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// The investment strategy this workflow follows (snapshot at transition time).
    pub strategy: String,
    /// Previous stage (`NULL` for the initial stage).
    pub from_stage: Option<String>,
    pub to_stage: String,
    pub note: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
