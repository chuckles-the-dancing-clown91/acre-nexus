//! A **mortgage / loan** secured against a property. A property can carry several
//! (e.g. a 1st and a 2nd lien); each optionally references the lender in the
//! counterparty registry ([`super::counterparty`]). Amounts are integer cents.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "mortgage")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    /// The lender, as a counterparty (`NULL` if not linked to the registry).
    pub lender_id: Option<Uuid>,
    /// `purchase` | `refinance` | `heloc` | `private` | `hard_money` |
    /// `seller_finance`.
    pub kind: String,
    /// Lien position (1 = first lien, 2 = second, …).
    pub position: i32,
    pub original_amount_cents: Option<i64>,
    pub current_balance_cents: Option<i64>,
    /// Interest rate in basis points (5.5% = 550).
    pub interest_rate_bps: Option<i32>,
    pub term_months: Option<i32>,
    /// Principal + interest payment, in cents/month.
    pub monthly_payment_cents: Option<i64>,
    /// Monthly escrow (taxes + insurance), in cents.
    pub escrow_monthly_cents: Option<i64>,
    pub start_date: Option<String>,
    pub maturity_date: Option<String>,
    pub loan_number: Option<String>,
    /// `active` | `paid_off` | `in_default`.
    pub status: String,
    pub notes: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
