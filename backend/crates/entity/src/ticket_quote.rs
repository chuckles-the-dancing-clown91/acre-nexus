//! A contractor's quote on a [`super::maintenance_ticket`] — amount +
//! description, approved/rejected by the same people who approve vendor
//! bills. Approval feeds the ticket's cost, and from there the vendor-bill
//! prefill.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ticket_quote")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub ticket_id: Uuid,
    /// The quoting contractor (counterparty).
    pub entity_id: Uuid,
    pub description: String,
    pub amount_cents: i64,
    /// `pending` | `approved` | `rejected`.
    pub status: String,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTimeWithTimeZone>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
