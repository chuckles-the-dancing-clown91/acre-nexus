//! Per-LLC **branding & signature** — exactly one row per LLC. This is what gets
//! merged into the LLC's email letters and generated lease/contract documents:
//! the logo (a reference to an `llc_document` of kind `logo`), brand colours, a
//! signature block, and reusable letterhead / footer verbiage.
//!
//! It cascades over the tenant-level `theme` (company-wide branding): an LLC that
//! leaves a field blank inherits the tenant default.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "llc_branding")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    #[sea_orm(unique)]
    pub llc_id: Uuid,
    /// The `llc_document` (kind = `logo`) used as this LLC's logo, if uploaded.
    pub logo_document_id: Option<Uuid>,
    /// Primary brand colour as a hex string, e.g. `#F5451F`.
    pub primary_color: Option<String>,
    pub accent_color: Option<String>,
    /// Name shown on the signature line (e.g. "Jane Doe, Managing Member").
    pub signature_name: Option<String>,
    pub signature_title: Option<String>,
    /// Free-form signature block (multi-line) appended to letters/contracts.
    pub signature_block: Option<String>,
    /// Letterhead / header verbiage rendered at the top of documents.
    pub letterhead: Option<String>,
    /// Footer / disclaimer verbiage rendered at the bottom of documents.
    pub footer: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
