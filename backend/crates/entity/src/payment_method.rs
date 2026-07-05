//! A **payment method** is a tokenized saved instrument (card or ACH bank
//! debit) a resident pays rent with. Only the provider's token and display
//! metadata are stored — PANs and account numbers never touch the platform
//! (PCI-safe by construction). A method may carry the lease's **autopay**
//! enrollment: at most one active autopay method per lease.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payment_method")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// The lease this method pays for, when attached by a resident.
    pub lease_id: Option<Uuid>,
    /// The portal user who saved it, when known.
    pub user_id: Option<Uuid>,
    /// `stripe` | `simulated`.
    pub provider: String,
    /// `card` | `ach`.
    pub kind: String,
    /// Provider token (`pm_…` / `btok_…` / `sim_…`) — never card data.
    pub external_id: String,
    /// Card brand or bank name, for display.
    pub brand: Option<String>,
    pub last4: String,
    pub exp_month: Option<i32>,
    pub exp_year: Option<i32>,
    /// `active` | `removed`.
    pub status: String,
    /// This method is the lease's autopay instrument.
    pub autopay: bool,
    /// Day of month autopay charges (clamped 1–28).
    pub autopay_day: Option<i32>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
