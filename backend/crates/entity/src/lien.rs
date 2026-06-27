//! A **lien** is an encumbrance recorded against a property's title — a mortgage,
//! tax lien, mechanic's lien, judgment, HOA lien, etc. This is the title-level
//! view of who has a claim; financing detail lives in [`super::mortgage`].

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "lien")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// The holder, as a counterparty, when known.
    pub lienholder_id: Option<Uuid>,
    pub lienholder_name: String,
    /// `mortgage` | `tax` | `mechanics` | `judgment` | `hoa` | `other`.
    pub kind: String,
    pub amount_cents: Option<i64>,
    /// Lien priority (1 = first).
    pub position: Option<i32>,
    pub recorded_date: Option<String>,
    /// `active` | `released`.
    pub status: String,
    /// Recording reference (instrument number).
    pub reference: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
