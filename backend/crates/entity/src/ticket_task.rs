//! One task on a work order ("Demo old surround", "Hang cement board"), with
//! the trade it needs and whether that trade is a contractor's.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ticket_task")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub ticket_id: Uuid,
    pub position: i32,
    pub title: String,
    /// `general` | `demo` | `plumbing` | `electrical` | `drywall` | `paint` |
    /// `tile` | `hvac` | `carpentry` | `roofing` | `flooring` | `cleaning` | …
    pub trade: String,
    pub est_minutes: Option<i32>,
    pub est_cost_cents: Option<i64>,
    /// Licensed work, or work the team doesn't do: send it to a vendor.
    pub needs_contractor: bool,
    /// The vendor (counterparty) doing this task, when it's contracted out.
    pub assignee_entity_id: Option<Uuid>,
    /// `todo` | `doing` | `done` | `skipped`.
    pub status: String,
    pub done_at: Option<DateTimeWithTimeZone>,
    pub done_by: Option<Uuid>,
    /// When the vendor was sent this task.
    pub dispatched_at: Option<DateTimeWithTimeZone>,
    /// A person on the team doing the task.
    pub assignee_user_id: Option<Uuid>,
    /// `partner` | `email`: how the task reached the vendor.
    pub dispatch_via: Option<String>,
    pub dispatch_note: Option<String>,
    /// SHA-256 of the link the vendor answers from (one per dispatch batch).
    pub vendor_token_hash: Option<String>,
    /// `accepted` | `declined` | `done`: the vendor's last answer.
    pub vendor_response: Option<String>,
    pub vendor_responded_at: Option<DateTimeWithTimeZone>,
    pub vendor_note: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
