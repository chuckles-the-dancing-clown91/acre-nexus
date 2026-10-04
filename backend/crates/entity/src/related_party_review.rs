//! A transaction between the family's own people and entities, held for a
//! market-rate note and a decision by someone who isn't a party to it.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "related_party_review")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `vendor_bill` | `lease` | `deal` | `other`.
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    /// The family entity (LLC) paying or receiving.
    pub entity_id: Option<Uuid>,
    pub counterparty_id: Option<Uuid>,
    pub summary: String,
    /// Why it was flagged.
    pub reason: String,
    pub amount_cents: Option<i64>,
    /// What the same thing costs at market, from the note.
    pub market_cents: Option<i64>,
    pub market_note: Option<String>,
    /// Owner ids who are parties to it; none of them may decide.
    pub parties: Json,
    /// `open` | `approved` | `rejected`.
    pub status: String,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTimeWithTimeZone>,
    pub decision_note: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
