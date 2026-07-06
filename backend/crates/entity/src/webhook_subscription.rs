//! A **webhook subscription** is a vendor's "subscribe, don't poll"
//! registration (issue #68): the API token it belongs to, the callback URL,
//! and the event types it wants — validated against the token's scopes so a
//! vendor can never subscribe to data it couldn't already read. Deliveries
//! are signed with the subscription's vaulted secret
//! (`webhook_sub.<id>.secret`, returned once at creation).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "webhook_subscription")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// The vendor token that owns this subscription.
    pub api_token_id: Uuid,
    pub url: String,
    /// Event-type strings, e.g. `["listing.updated", "payment.recorded"]`.
    pub event_types: Json,
    /// Vault key of the HMAC signing secret (never stored in plaintext).
    pub secret_ref: String,
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
