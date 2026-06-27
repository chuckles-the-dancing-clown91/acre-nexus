//! A **background job** is a unit of asynchronous, automated work driven by the
//! Tokio scheduler (`api::scheduler`) — e.g. running a tenant-screening
//! background check, awaiting an external callback, or sending an automated
//! email at a future time.
//!
//! This is the persistence layer for the "progress automation steps" described
//! in the product brief: durable, resumable, and observable.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "background_job")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// Job kind: `background_check` | `auto_email` | `screening` | `webhook_wait`.
    pub kind: String,
    /// `pending` | `running` | `awaiting_callback` | `completed` | `failed`.
    pub status: String,
    /// Arbitrary job-specific payload.
    pub payload: Json,
    /// Result / error detail once resolved.
    pub result: Option<Json>,
    /// Earliest time the scheduler should pick this job up.
    pub run_at: DateTimeWithTimeZone,
    pub attempts: i32,
    /// Retry budget: after this many failed attempts the job moves to `failed`.
    pub max_attempts: i32,
    /// Last error message, set when an attempt fails (for observability).
    pub last_error: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
