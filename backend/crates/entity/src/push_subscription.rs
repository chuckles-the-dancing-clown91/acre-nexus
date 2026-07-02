//! One browser **Web Push subscription**: the push-service endpoint plus the
//! client's P-256 public key and auth secret, exactly as
//! `PushManager.subscribe()` hands them over. A user may hold several (one per
//! browser/device); expired subscriptions (HTTP 404/410 from the push
//! service) are pruned automatically on send.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "push_subscription")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    /// Push-service URL this subscription delivers through (unique).
    pub endpoint: String,
    /// Client public key (base64url, uncompressed P-256 point).
    pub p256dh: String,
    /// Client auth secret (base64url, 16 bytes).
    pub auth: String,
    pub user_agent: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
