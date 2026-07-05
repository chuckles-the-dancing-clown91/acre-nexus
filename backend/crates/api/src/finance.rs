//! **Financial time-series** for the dashboards (roadmap Phase 3, issue #39).
//!
//! The series merges two sources: flow metrics (rent due/collected, NOI) are
//! computed live from the payments table and the ledger for every month in
//! range, while point-in-time metrics (occupancy, delinquency, portfolio
//! value) come from the monthly [`entity::financial_snapshot`] history — with
//! the current month always computed fresh so the dashboard never lags the
//! books.

use chrono::{Datelike, NaiveDate, Utc};
use entity::prelude::{FinancialSnapshot, LedgerEntry, LedgerTxn};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use std::collections::HashMap;
use uuid::Uuid;

/// One month of the finance series.
#[derive(Clone, Debug)]
pub struct MonthPoint {
    /// `YYYY-MM`.
    pub month: String,
    pub rent_due_cents: i64,
    pub rent_collected_cents: i64,
    pub noi_cents: i64,
    pub occupancy_bps: i32,
    pub delinquency_bps: i32,
    pub portfolio_value_cents: i64,
    pub active_leases: i32,
}

/// The trailing `months` months, oldest first, ending with the current month.
pub fn month_keys(today: NaiveDate, months: u32) -> Vec<String> {
    let months = months.clamp(1, 60);
    let mut keys = Vec::with_capacity(months as usize);
    let (mut y, mut m) = (today.year(), today.month());
    for _ in 0..months {
        keys.push(format!("{y:04}-{m:02}"));
        if m == 1 {
            y -= 1;
            m = 12;
        } else {
            m -= 1;
        }
    }
    keys.reverse();
    keys
}

/// Income − expenses posted to any of the tenant's ledgers in `YYYY-MM`.
pub async fn month_noi(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    month: &str,
) -> Result<i64, sea_orm::DbErr> {
    let prefix = format!("{month}-");
    let txn_ids: Vec<Uuid> = LedgerTxn::find()
        .filter(entity::ledger_txn::Column::TenantId.eq(tenant_id))
        .filter(entity::ledger_txn::Column::TxnDate.starts_with(&prefix))
        .all(db)
        .await?
        .into_iter()
        .map(|t| t.id)
        .collect();
    if txn_ids.is_empty() {
        return Ok(0);
    }
    let entries = LedgerEntry::find()
        .filter(entity::ledger_entry::Column::TenantId.eq(tenant_id))
        .filter(entity::ledger_entry::Column::TxnId.is_in(txn_ids))
        .all(db)
        .await?;
    // Resolve account kinds once.
    let account_ids: Vec<Uuid> = entries.iter().map(|e| e.account_id).collect();
    let kinds: HashMap<Uuid, String> = entity::prelude::LedgerAccount::find()
        .filter(entity::ledger_account::Column::TenantId.eq(tenant_id))
        .filter(entity::ledger_account::Column::Id.is_in(account_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|a| (a.id, a.kind))
        .collect();

    let mut income = 0i64;
    let mut expenses = 0i64;
    for e in &entries {
        let Some(kind) = kinds.get(&e.account_id) else {
            continue;
        };
        let signed = if e.side == "credit" {
            e.amount_cents
        } else {
            -e.amount_cents
        };
        match kind.as_str() {
            "income" => income += signed,
            // Expenses are debit-normal: flip the sign.
            "expense" => expenses -= signed,
            _ => {}
        }
    }
    Ok(income - expenses)
}

/// Build the merged series for a tenant.
pub async fn series(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    months: u32,
) -> Result<Vec<MonthPoint>, sea_orm::DbErr> {
    let today = Utc::now().date_naive();
    let keys = month_keys(today, months);
    let current_key = keys.last().cloned().unwrap_or_default();

    let snapshots: HashMap<String, entity::financial_snapshot::Model> = FinancialSnapshot::find()
        .filter(entity::financial_snapshot::Column::TenantId.eq(tenant_id))
        .filter(entity::financial_snapshot::Column::Month.is_in(keys.clone()))
        .all(db)
        .await?
        .into_iter()
        .map(|s| (s.month.clone(), s))
        .collect();

    // The current month is always live.
    let live = crate::billing::compute_point_in_time(db, tenant_id).await?;

    let mut points = Vec::with_capacity(keys.len());
    for key in &keys {
        let (rent_due, rent_collected) =
            crate::billing::month_rent_figures(db, tenant_id, key).await?;
        let noi = month_noi(db, tenant_id, key).await?;
        let snap = snapshots.get(key);
        let is_current = *key == current_key;
        points.push(MonthPoint {
            month: key.clone(),
            rent_due_cents: rent_due,
            rent_collected_cents: rent_collected,
            noi_cents: noi,
            occupancy_bps: if is_current {
                live.occupancy_bps
            } else {
                snap.map(|s| s.occupancy_bps).unwrap_or(0)
            },
            delinquency_bps: if is_current {
                live.delinquency_bps
            } else {
                snap.map(|s| s.delinquency_bps).unwrap_or(0)
            },
            portfolio_value_cents: if is_current {
                live.portfolio_value_cents
            } else {
                snap.map(|s| s.portfolio_value_cents).unwrap_or(0)
            },
            active_leases: if is_current {
                live.active_leases
            } else {
                snap.map(|s| s.active_leases).unwrap_or(0)
            },
        });
    }
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_keys_walk_backwards_across_year_boundaries() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 15).unwrap();
        let keys = month_keys(today, 4);
        assert_eq!(keys, vec!["2025-11", "2025-12", "2026-01", "2026-02"]);
    }

    #[test]
    fn month_keys_clamp_range() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 4).unwrap();
        assert_eq!(month_keys(today, 0).len(), 1);
        assert_eq!(month_keys(today, 200).len(), 60);
        assert_eq!(month_keys(today, 1), vec!["2026-07"]);
    }
}
