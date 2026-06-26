//! Tenant-facing module management: list available modules with their enabled
//! state, and toggle a module on/off. These power the "Modules" section of a
//! tenant's software settings. Gated by `tenant:manage`.

pub mod dto;
pub mod list;
pub mod set;
