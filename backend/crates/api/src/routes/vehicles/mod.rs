//! **Vehicle** endpoints — resident vehicle profiles, attachable to an
//! application, a lease, and/or a renter user. Garage/parking amenities pull these
//! into the generated lease document.

pub mod create;
pub mod delete;
pub mod dto;
pub mod list;
pub mod update;
