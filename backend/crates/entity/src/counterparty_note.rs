//! A timestamped **note** on a [`super::counterparty`] — the running log of
//! interactions (e.g. "spoke with loan officer about the refi rate"). Append-only
//! in practice; many notes per counterparty.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "counterparty_note")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub counterparty_id: Uuid,
    /// The user who wrote the note (`NULL` for system-generated notes).
    pub author_user_id: Option<Uuid>,
    pub body: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
