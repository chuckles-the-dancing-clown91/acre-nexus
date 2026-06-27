//! Entities registry — counterparties (banks, lenders, contractors, insurers,
//! title companies, …) a tenant transacts with, plus the running notes about
//! them. Tenant-scoped CRUD-ish surface guarded by `entity:read`/`entity:manage`.

pub mod add_note;
pub mod create;
pub mod dto;
pub mod get;
pub mod list;
pub mod update;
