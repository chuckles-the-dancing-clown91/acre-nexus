//! A **lease payment** is one entry in a lease's rent ledger — a receivable
//! that is due and, once collected, how it settled. Since Phase 3 it carries
//! the full payment lifecycle: a `kind` (rent/deposit/fee), the charging
//! method + provider ids when collected electronically, the receipt number,
//! and the double-entry `ledger_txn` its settlement posted. Together these
//! drive a lease's `payment_status` and outstanding `balance_cents`.

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
    /// `due` | `processing` | `paid` | `failed` | `late` | `partial`.
    pub status: String,
    /// `ach` | `card` | `check` | `cash` | … (free-form).
    pub method: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    /// `rent` | `deposit` | `fee` | `application_fee` | `other`.
    pub kind: String,
    /// The saved [`crate::payment_method`] that charged it, if electronic.
    pub method_id: Option<Uuid>,
    /// `stripe` | `simulated` when collected through a processor.
    pub provider: Option<String>,
    /// Processor payment id (`pi_…` / `sim_pi_…`).
    pub external_id: Option<String>,
    pub failure_reason: Option<String>,
    /// Receipt number stamped at settlement (`RCT-…`).
    pub receipt_number: Option<String>,
    /// The balanced ledger posting recorded at settlement.
    pub ledger_txn_id: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
