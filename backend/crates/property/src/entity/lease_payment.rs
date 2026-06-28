//! A **lease payment** is one entry in a lease's rent ledger — a charge that is
//! due and, once paid, when/how it was settled. Together these drive a lease's
//! `payment_status` and outstanding `balance_cents`.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "lease_payment")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    pub due_date: String,
    pub amount_cents: i64,
    pub paid_date: Option<String>,
    /// `due` | `paid` | `late` | `partial`.
    pub status: String,
    /// `ach` | `card` | `check` | `cash` | … (free-form).
    pub method: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
