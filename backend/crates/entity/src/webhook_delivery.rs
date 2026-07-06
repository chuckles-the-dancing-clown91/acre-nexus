//! A **webhook delivery** is one event dispatched to one subscriber: the
//! payload, the attempt count, and the outcome (`pending → delivered |
//! dead`). This is the observability surface a vendor reads to see failures
//! — and the record a replay copies from. Delivery itself rides the retrying
//! job queue (`webhook_deliver`), so attempts get the platform's standard
//! backoff and dead-letter semantics.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "webhook_delivery")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub subscription_id: Uuid,
    pub event_type: String,
    pub payload: Json,
    /// `pending` | `delivered` | `dead`.
    pub status: String,
    pub attempts: i32,
    /// Last HTTP status from the subscriber, when one answered.
    pub response_status: Option<i32>,
    pub last_error: Option<String>,
    pub delivered_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
