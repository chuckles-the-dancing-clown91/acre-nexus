//! Per-property investment **workflow** endpoints (flip / rental / BRRRR / …).
//! Stage templates come from [`crate::workflow`]; transitions are recorded in
//! `workflow_event`. Mounted by the `properties` module.

pub mod advance;
pub mod dto;
pub mod get;
