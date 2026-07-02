//! E-signature envelope routes (roadmap Phase 2).
//!
//! Console: create/send an envelope for a lease document, inspect it (signers
//! and audit trail), remind pending signers, void. Public: the tokenized
//! signer endpoints the emailed/texted signing links hit (view → sign or
//! decline) — possession of the token is the credential, like a presigned URL.

pub mod create;
pub mod dto;
pub mod get;
pub mod public;
pub mod remind;
pub mod void;

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

/// An envelope's audit-trail events, newest first.
pub async fn envelope_events(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    envelope_id: Uuid,
) -> Result<Vec<entity::esign_event::Model>, sea_orm::DbErr> {
    entity::prelude::EsignEvent::find()
        .filter(entity::esign_event::Column::TenantId.eq(tenant_id))
        .filter(entity::esign_event::Column::EnvelopeId.eq(envelope_id))
        .order_by_desc(entity::esign_event::Column::CreatedAt)
        .all(db)
        .await
}

/// Envelope states that still accept signatures.
pub fn is_open(status: &str) -> bool {
    matches!(status, "sent" | "partially_signed")
}
