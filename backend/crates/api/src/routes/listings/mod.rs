//! Console **listing management** — the first step of the leasing pipeline:
//! advertise a property (create/publish), keep it current (update), and let
//! the pipeline retire it automatically (`Pending` on application→lease
//! conversion, `Leased` + unpublished when the lease activates — see
//! [`crate::listing_sync`]).

pub mod create;
pub mod dto;
pub mod list;
pub mod update;

/// Listing statuses the console may set (the pipeline sets `Pending`/`Leased`
/// itself, but staff can correct state by hand).
pub const STATUSES: &[&str] = &["Available", "New", "Pending", "Leased"];
