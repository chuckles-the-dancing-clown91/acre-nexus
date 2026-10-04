//! A visit someone has to be there for: a repair, a showing, an inspection.
//! Staff offer windows; the other side picks one, or proposes another.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "appointment")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    /// repair | showing | inspection | other
    pub kind: String,
    /// ticket | task | lead | tour | custom
    pub subject_type: String,
    pub subject_id: Option<Uuid>,
    pub title: String,
    /// proposed | confirmed | declined | cancelled | done | no_show
    pub status: String,
    /// Offered windows: `[{ "start": iso, "end": iso }]`.
    pub windows: Json,
    pub starts_at: Option<DateTimeWithTimeZone>,
    pub ends_at: Option<DateTimeWithTimeZone>,
    pub with_name: Option<String>,
    pub with_email: Option<String>,
    pub with_phone: Option<String>,
    /// resident | prospect | vendor | owner
    pub with_role: String,
    pub assignee_user_id: Option<Uuid>,
    pub vendor_entity_id: Option<Uuid>,
    pub note: Option<String>,
    pub access_notes: Option<String>,
    /// SHA-256 of the one-time link token.
    pub token_hash: Option<String>,
    /// resident | prospect | vendor | staff
    pub confirmed_by: Option<String>,
    pub confirmed_at: Option<DateTimeWithTimeZone>,
    /// A time the other side asked for instead of the windows.
    pub proposed_start: Option<DateTimeWithTimeZone>,
    pub proposed_end: Option<DateTimeWithTimeZone>,
    /// Reminder lead hours already sent, e.g. `[24, 2]`.
    pub reminded: Json,
    pub outcome_note: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
