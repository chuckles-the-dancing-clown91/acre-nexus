//! Rentals & leasing — tenant-scoped CRUD over a property's units, its leases,
//! and the per-lease rent payment ledger, with USD labels for display.

pub mod create_lease;
pub mod create_unit;
pub mod dto;
pub mod get_lease;
pub mod list_leases;
pub mod list_payments;
pub mod list_property_leases;
pub mod list_units;
pub mod record_payment;
pub mod update_lease;
pub mod update_unit;
