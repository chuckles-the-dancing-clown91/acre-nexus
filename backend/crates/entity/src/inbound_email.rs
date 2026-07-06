//! An **inbound email** is one message received at a tenant's inbound
//! address, logged with where it was routed — a maintenance-ticket comment, a
//! CRM lead, or unmatched. This is the inbound half of the communication
//! history (outbound sends live in [`super::notification`]).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inbound_email")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub from_email: String,
    pub to_email: String,
    pub subject: String,
    pub body_text: String,
    /// `ticket_comment` | `lead` | `unmatched`.
    pub routed: String,
    /// The ticket-comment or lead row the message landed on.
    pub routed_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
