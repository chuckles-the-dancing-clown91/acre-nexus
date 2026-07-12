//! A **lease renewal** is a proposed change of terms on an existing tenancy
//! (issue #44) — typically a rent increase and an extended end date. It rides
//! the Phase 2 document/e-sign substrate: propose → generate an addendum
//! ([`super::lease_document`]) → send it out as an [`super::esign_envelope`]
//! (`purpose = "renewal"`) → on completion the new terms are applied to the
//! underlying [`super::lease`] and the renewal is marked `activated`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "lease_renewal")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    /// `proposed` | `sent` | `signed` | `activated` | `declined` | `cancelled`.
    pub status: String,
    /// The rent this renewal moves *from*, in cents (pinned at propose time).
    pub current_rent_cents: i64,
    /// The rent it moves *to*, in cents.
    pub new_rent_cents: i64,
    /// `YYYY-MM-DD` — effective date of the renewed term.
    pub new_start_date: String,
    /// `YYYY-MM-DD`, or `None` for month-to-month.
    pub new_end_date: Option<String>,
    pub term_months: Option<i32>,
    pub notes: Option<String>,
    /// The generated addendum document and the envelope it was sent in.
    pub lease_document_id: Option<Uuid>,
    pub envelope_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub activated_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
