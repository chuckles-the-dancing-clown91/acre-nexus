//! The **ESIGN/UETA audit trail** for an e-signature envelope: an append-only
//! log of every meaningful act (sent, viewed, signed, declined, reminded,
//! completed, voided) with the acting signer, source IP, and user agent.
//! Together with the envelope's `body_hash` this is the tamper-evident record
//! that makes the electronic signature legally defensible.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "esign_event")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub envelope_id: Uuid,
    /// The signer this event belongs to; `None` for envelope-level events.
    pub signer_id: Option<Uuid>,
    /// `sent` | `viewed` | `signed` | `declined` | `reminded` | `completed` | `voided`.
    pub event: String,
    /// Structured context (signer name, reason, …) — never the document body.
    pub detail: Json,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
