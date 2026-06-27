//! # Property enrichment engine
//!
//! The automation behind "Zillow but better": background workers that fetch and
//! validate external property data — parcel/county records, tax history, an AVM
//! valuation, schools, and utilities — and persist it to the property-data
//! tables. The trail of attempts lives in `enrichment_run`.
//!
//! ## How it runs
//! Work is driven by the durable Tokio job queue ([`crate::scheduler`]). A
//! `enrich_property` orchestrator job fans out into one child job per [`Source`]
//! (`enrich_geocode`, `enrich_parcel`, …), so each source runs — and retries with
//! backoff — independently. The [`crate::modules::enrichment`] module owns these
//! job kinds and calls [`runner::run_source`] for each.
//!
//! ## Providers
//! Every source sits behind the same interface. [`geocode`] is a **live** call to
//! the free U.S. Census geocoder (proving real outbound validation); the rest are
//! deterministic [`simulated`] providers so the state machine and durability are
//! real while CI stays hermetic. Swapping in a real provider is a one-function
//! change.
//!
//! Each concern is its own small file: [`source`] (taxonomy), [`data`] (provider
//! output shapes + error), [`geocode`] (live), [`simulated`] (stand-ins), and
//! [`runner`] (persist + summarise).

pub mod data;
pub mod geocode;
pub mod runner;
pub mod simulated;
pub mod source;

pub use source::{Source, JOB_KINDS, ORCHESTRATOR_KIND};
