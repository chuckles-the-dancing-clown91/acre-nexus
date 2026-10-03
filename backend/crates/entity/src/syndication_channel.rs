//! A rental portal the workspace's listings go out to (Zillow's rental network,
//! or the MITS feed Apartments.com and others read), by way of a feed the
//! portal pulls from a secret URL.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "syndication_channel")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `zillow` | `mits`.
    pub channel: String,
    pub enabled: bool,
    /// The secret in the feed URL.
    pub feed_token: String,
    /// Who renters reach, shown on every listing in the feed.
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub last_pulled_at: Option<DateTimeWithTimeZone>,
    pub last_pull_agent: Option<String>,
    pub pull_count: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
