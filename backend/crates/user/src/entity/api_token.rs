//! A scoped, revocable **API token** for the vendor API. The raw secret is shown
//! once at creation; only a SHA-256 `token_hash` is stored. `prefix` is the
//! human-visible first segment for identification in dashboards.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "api_token")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    /// Displayable prefix, e.g. `acre_live_a1b2c3`.
    pub prefix: String,
    #[serde(skip_serializing)]
    pub token_hash: String,
    /// JSON array of permission scopes the token may exercise.
    pub scopes: Json,
    pub last_used_at: Option<DateTimeWithTimeZone>,
    pub expires_at: Option<DateTimeWithTimeZone>,
    pub revoked_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
