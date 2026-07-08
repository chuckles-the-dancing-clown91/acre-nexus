//! An **acquisition deal**: a prospective property moving through the buy-side
//! pipeline (`prospecting → offer → under_contract → closing → owned`) before it
//! becomes a fully-onboarded [`crate::property`].
//!
//! A deal carries its **underwriting assumptions** (purchase / rehab / rent /
//! financing / projection knobs) so the cap-rate / cash-on-cash / IRR / DSCR
//! calculators recompute deterministically, and a JSON **due-diligence
//! checklist**. Supporting files (offers, LOIs, inspection reports) live in the
//! polymorphic [`crate::document`] service with `owner_type = "deal"` — the deal
//! data room. Money is integer cents; rates are basis points.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "deal")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub address: String,
    pub city: String,
    /// Pipeline stage: `prospecting` | `offer` | `under_contract` | `closing` |
    /// `owned` | `dead` (see `api::deals`).
    pub stage: String,
    /// Exit strategy driving the eventual property workflow: `flip` | `brrrr` |
    /// `rental` | `hold` | `wholesale`.
    pub strategy: String,
    pub property_type: Option<String>,
    /// How the deal was sourced (`mls` | `wholesaler` | `direct` | `auction` …).
    pub source: Option<String>,
    /// Listing broker / agent, resolved to a [`crate::counterparty`].
    pub broker_id: Option<Uuid>,
    pub notes: Option<String>,

    // ---- Offer terms ----
    pub asking_price_cents: Option<i64>,
    pub offer_price_cents: Option<i64>,
    pub earnest_money_cents: Option<i64>,
    /// Target close date (`YYYY-MM-DD`).
    pub target_close_on: Option<String>,

    // ---- Underwriting assumptions ----
    /// After-repair value.
    pub arv_cents: Option<i64>,
    pub rehab_budget_cents: Option<i64>,
    pub closing_costs_cents: Option<i64>,
    pub est_monthly_rent_cents: Option<i64>,
    /// Monthly operating expenses (taxes/insurance/mgmt/maintenance), excl. debt.
    pub est_monthly_expenses_cents: Option<i64>,
    /// Vacancy allowance, basis points of gross rent.
    pub vacancy_bps: Option<i32>,
    /// Down payment, basis points of the purchase price.
    pub down_payment_bps: Option<i32>,
    pub interest_rate_bps: Option<i32>,
    pub loan_term_years: Option<i32>,
    /// Annual rent (and expense) growth used in the multi-year projection.
    pub rent_growth_bps: Option<i32>,
    /// Annual property appreciation (exit basis when no exit cap is set).
    pub appreciation_bps: Option<i32>,
    /// Exit capitalisation rate used to value the property at sale.
    pub exit_cap_rate_bps: Option<i32>,
    /// Cost of sale (broker + closing), basis points of the sale price.
    pub selling_costs_bps: Option<i32>,
    pub hold_years: Option<i32>,

    /// Due-diligence checklist: a JSON array of `{ key, label, done, note }`.
    pub checklist: Json,
    /// Set once the deal is converted into an owned [`crate::property`].
    pub converted_property_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
