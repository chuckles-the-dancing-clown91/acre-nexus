//! A move-in / move-out inspection on a lease — a checklist of condition
//! items ([`super::inspection_item`]) with photos riding the document service
//! (`owner_type = "inspection"`).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inspection")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    /// `move_in` | `move_out`.
    pub kind: String,
    /// `draft` | `completed`.
    pub status: String,
    /// ISO date (`YYYY-MM-DD`), like `lease.start_date`.
    pub scheduled_date: Option<String>,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub completed_by: Option<Uuid>,
    pub notes: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
