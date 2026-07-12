//! An **e-signature envelope**: one lease document sent out for signature to a
//! set of [`super::esign_signer`]s. The envelope tracks the collective signing
//! state (`sent` → `partially_signed` → `completed`, or `declined` / `voided`)
//! and pins a SHA-256 of the document body at send time so every signer
//! provably saw the same text. When the last signer signs, the completed,
//! signed rendition is stored in the document service
//! (`signed_document_id`) and the underlying lease activates.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "esign_envelope")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    /// The [`super::lease_document`] this envelope sends for signature.
    pub lease_document_id: Uuid,
    pub title: String,
    /// Optional note from the sender, shown to signers.
    pub message: Option<String>,
    /// `sent` | `partially_signed` | `completed` | `declined` | `voided`.
    pub status: String,
    /// `lease` (initial lease signing → activates the tenancy, the default) |
    /// `renewal` (a renewal addendum → bumps the tenancy's rent + term on
    /// completion).
    pub purpose: String,
    /// SHA-256 (hex) of the document body at send time — all signers sign
    /// exactly this text.
    pub body_hash: String,
    /// The signed PDF stored in the document service once completed.
    pub signed_document_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub sent_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub voided_at: Option<DateTimeWithTimeZone>,
    pub void_reason: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
