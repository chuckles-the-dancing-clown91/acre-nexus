//! Property Intelligence endpoints — the rich per-property data ("Zillow but
//! better") and the controls to (re)fetch it via the enrichment engine.
//!
//! One handler per file; shared shapes in [`dto`]. Mounted by the
//! `property_intel` module ([`crate::modules::enrichment`]).

pub mod dto;
pub mod enrich;
pub mod get_intel;
pub mod list_enrichment;
