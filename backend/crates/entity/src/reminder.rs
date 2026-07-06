//! A **reminder** is one entry in the cross-cutting scheduling engine: a
//! subject (lease renewal, license / insurance expiry, tour, inspection, or
//! anything custom), a due date, and the lead times at which to notify. The
//! per-tenant `reminder_scan` job (see `api::reminders`) fires notifications
//! through the notification substrate at each configured lead time and
//! records which leads have fired, so a reminder never double-sends.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reminder")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `lease` | `license` | `insurance` | `tour` | `inspection` | `custom`.
    pub subject_type: String,
    /// The subject row (lease id, …) when one exists.
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    /// `YYYY-MM-DD`.
    pub due_date: String,
    /// Days before the due date to notify, e.g. `[30, 7, 1]`.
    pub lead_days: Json,
    /// External recipient email addresses; staff holding `calendar:read` are
    /// always notified in-app/push.
    pub recipients: Json,
    /// Lead times that have already fired.
    pub fired: Json,
    /// `active` | `done` | `cancelled`.
    pub status: String,
    pub completed_at: Option<DateTimeWithTimeZone>,
    /// `None` = the pipeline created it (lease renewal sync).
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
