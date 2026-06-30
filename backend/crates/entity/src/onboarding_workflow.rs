//! The **onboarding workflow** is one resumable, audited setup record per tenant.
//! `state` holds the furthest-reached step; `steps` is a JSON map of per-step
//! completion + metadata (recomputed from the live database on read). The state
//! machine and its completion predicates live in
//! `api::routes::onboarding::state`.
//!
//! ```text
//! provisioning → firm_admin_accepted → branding_configured → domains_configured
//!   → entities_created → banking_linked → portfolio_imported → staff_invited → live
//! ```

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "onboarding_workflow")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// Furthest-reached state key (see the module-level diagram).
    pub state: String,
    /// Per-step completion + metadata, recomputed on read.
    pub steps: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
