//! A **financial snapshot** is one month's dashboard rollup for a tenant:
//! occupancy, delinquency, portfolio value, rent due/collected, and NOI.
//! Captured (upserted) by the billing cycle because point-in-time metrics like
//! occupancy cannot be derived retroactively; the finance series endpoint
//! merges these with live ledger rollups to chart trends.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "financial_snapshot")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    /// `YYYY-MM`; unique per tenant.
    pub month: String,
    /// Occupied units / total units, in basis points (10000 = 100%).
    pub occupancy_bps: i32,
    /// Delinquent active leases / active leases, in basis points.
    pub delinquency_bps: i32,
    pub portfolio_value_cents: i64,
    pub rent_due_cents: i64,
    pub rent_collected_cents: i64,
    /// Income − expenses posted to the ledger in the month.
    pub noi_cents: i64,
    pub active_leases: i32,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
