//! One step of a [`super::process`], with its place in the order, who owns it,
//! what it waits on, and where it stands.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "process_step")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub process_id: Uuid,
    pub position: i32,
    pub key: String,
    pub title: String,
    pub description: Option<String>,
    /// `office` | `maintenance` | `vendor` | `leasing` | `owner`.
    pub owner_role: String,
    pub assignee_user_id: Option<Uuid>,
    /// JSON array of step keys this step waits on.
    pub depends_on: Json,
    /// ISO date it is due.
    pub due_on: Option<String>,
    pub required: bool,
    pub requires_photo: bool,
    /// `blocked` | `ready` | `doing` | `done` | `skipped`.
    pub status: String,
    pub started_at: Option<DateTimeWithTimeZone>,
    pub done_at: Option<DateTimeWithTimeZone>,
    pub done_by: Option<Uuid>,
    pub skip_reason: Option<String>,
    pub note: Option<String>,
    /// The work order this step opened (its cost counts toward the process).
    pub ticket_id: Option<Uuid>,
    pub cost_cents: Option<i64>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
