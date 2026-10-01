//! Time off (`vacation` | `sick` | `personal` | `unpaid`), `pending` until reviewed. Dates are inclusive `YYYY-MM-DD`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "time_off_request")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub starts_on: String,
    pub ends_on: String,
    pub kind: String,
    /// `pending` | `approved` | `denied` | `cancelled`
    pub status: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub reason: Option<String>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTimeWithTimeZone>,
    pub review_note: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
