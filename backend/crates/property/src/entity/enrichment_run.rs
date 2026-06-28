//! An **enrichment run** records one automated attempt to fetch + validate a
//! single data source for a property (parcel, tax, valuation, schools, utilities,
//! or geocode). It is the observable audit trail for the enrichment engine: which
//! source, driven by which background job, with what outcome.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "enrichment_run")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// Data source: `geocode` | `parcel` | `tax` | `valuation` | `schools` |
    /// `utilities` | `orchestrator`.
    pub source: String,
    /// `succeeded` | `failed`.
    pub status: String,
    /// The background job that produced this run, when applicable.
    pub job_id: Option<Uuid>,
    /// `simulated` or the live provider name (e.g. `census_geocoder`).
    pub provider: String,
    /// Result summary or error detail.
    pub detail: Option<Json>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
