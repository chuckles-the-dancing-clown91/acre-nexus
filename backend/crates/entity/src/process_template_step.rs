//! One step of a [`super::process_template`]: what to do, who does it, which
//! steps it waits on, and how it is checked off.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "process_template_step")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub template_id: Uuid,
    pub position: i32,
    /// Stable id inside the template; other steps' `depends_on` name it.
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    /// `office` | `maintenance` | `vendor` | `leasing` | `owner`.
    pub owner_role: String,
    /// JSON array of step keys this step waits on.
    pub depends_on: Json,
    /// Days after the process starts that this step is due.
    pub due_offset_days: i32,
    /// A required step must be done (or skipped with a reason) before the
    /// process can finish.
    pub required: bool,
    pub requires_photo: bool,
    /// When set, "open a work order" is offered on the step with this category.
    pub ticket_category: Option<String>,
    pub ticket_priority: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
