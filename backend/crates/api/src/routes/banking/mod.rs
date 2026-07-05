//! **Banking** endpoints — `operating` / `trust` accounts scoped to a legal
//! entity (§6). Trust accounts are subject to the commingling invariant
//! (`crate::accounting`).

pub mod create;
pub mod dto;
pub mod feed;
pub mod list;
