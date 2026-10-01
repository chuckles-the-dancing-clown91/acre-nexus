//! A **tour request**: a prospect asking to see a home from the public site.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tour_request")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub listing_id: Option<Uuid>,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub preferred_times: Option<String>,
    pub message: Option<String>,
    /// The prospect agreed to be contacted about this request.
    pub consent: bool,
    /// `new` | `contacted` | `scheduled` | `closed`.
    pub status: String,
    pub note: Option<String>,
    pub handled_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
