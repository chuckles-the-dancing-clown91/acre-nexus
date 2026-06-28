//! A **rendered** document produced from an `llc_template` + branding: a lease
//! contract or a tenant letter, materialised to PDF and stored in the object
//! store. Kept as a row so the document is durable, auditable, and re-downloadable
//! (and, for leases, immutable once finalised even if the template later changes).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "generated_document")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub llc_id: Uuid,
    /// The template this was rendered from, if any.
    pub template_id: Option<Uuid>,
    /// The lease this contract belongs to (cross-table within property_db), if any.
    pub lease_id: Option<Uuid>,
    /// `lease` | `letter`.
    pub kind: String,
    pub title: String,
    /// Storage backend + key for the rendered PDF bytes.
    pub storage_provider: String,
    pub storage_key: String,
    pub mime_type: String,
    pub size_bytes: i64,
    /// `draft` | `final` | `sent` | `signed`.
    pub status: String,
    pub rendered_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
