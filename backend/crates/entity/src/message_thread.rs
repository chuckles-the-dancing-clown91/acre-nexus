//! A resident ↔ manager conversation on a lease — the renter portal's
//! "message the manager" surface. Messages hang off the thread.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "message_thread")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    pub property_id: Uuid,
    pub subject: String,
    /// `open` | `closed`.
    pub status: String,
    pub created_by: Uuid,
    pub last_message_at: DateTimeWithTimeZone,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
