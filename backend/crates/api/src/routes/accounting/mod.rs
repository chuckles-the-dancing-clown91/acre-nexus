//! **Accounting** endpoints — the chart of accounts, the double-entry
//! journal, financial reports (trial balance, income statement, trust
//! reconciliation), and the dashboard finance series. Reads gate on
//! `ledger:read`; manual journal entries and account management on
//! `ledger:manage`.

pub mod accounts;
pub mod dto;
pub mod reports;
pub mod series;
pub mod transactions;
