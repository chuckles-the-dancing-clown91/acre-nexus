//! A to-do on a property or anything on it: a permit to close out, a policy
//! to renew, a school zone to confirm, a warranty to claim.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "action_item")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// property | parcel | permit | insurance | school | asset | utility | tax | document | unit
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub notes: Option<String>,
    pub due_on: Option<String>,
    /// low | normal | high
    pub priority: String,
    /// open | done | dismissed
    pub status: String,
    pub assignee_user_id: Option<Uuid>,
    /// Set when the item came from a suggestion, so it isn't suggested twice.
    pub suggestion_key: Option<String>,
    pub created_by: Option<Uuid>,
    pub completed_at: Option<DateTimeWithTimeZone>,
    pub completed_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
