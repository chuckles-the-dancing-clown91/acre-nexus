//! LLC (holding-entity) endpoints — tenant-scoped.
//!
//! One handler per file; shared request/response shapes live in [`dto`]. The
//! module that mounts these routes references them by path (`llcs::list::list`).

pub mod create;
pub mod dto;
pub mod list;
