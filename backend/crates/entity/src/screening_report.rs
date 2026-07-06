//! An FCRA **screening report**: one background check per application —
//! credit + criminal + eviction — ordered through the screening provider
//! (Checkr live, deterministic simulation otherwise) with the applicant's
//! consent stamped at order time. The row holds the display summary and the
//! final policy verdict; a live provider's full report artifact belongs in
//! the document service, never in this table.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "screening_report")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub application_id: Uuid,
    /// Provider key (`checkr`).
    pub provider: String,
    /// Provider report id once ordered.
    pub external_id: Option<String>,
    /// `pending` | `in_progress` | `complete` | `failed`.
    pub status: String,
    pub include_credit: bool,
    pub include_criminal: bool,
    pub include_eviction: bool,
    /// The applicant's screening consent, copied from the application at
    /// order time (FCRA §604(b): no report without written permission).
    pub consent_at: Option<DateTimeWithTimeZone>,
    pub credit_score: Option<i32>,
    pub criminal_records: Option<i32>,
    pub eviction_records: Option<i32>,
    /// Provider recommendation: `clear` | `consider`.
    pub recommendation: Option<String>,
    /// Final policy verdict landed on the application: `cleared` | `failed`.
    pub result: Option<String>,
    /// Policy trips + record findings (JSON array of strings).
    pub reasons: Option<Json>,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
