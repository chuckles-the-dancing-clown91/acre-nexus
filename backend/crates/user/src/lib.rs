//! # Acre **user** domain
//!
//! Identity, authentication, RBAC, tenancy and platform configuration — the
//! tables hosted in the `acre_user` database. Also home to the two genuinely
//! cross-cutting platform tables (`audit_log`, `background_job`), which live
//! here so the platform keeps exactly three databases (user / property /
//! client) rather than a separate fourth.
//!
//! Money is stored as integer cents (`i64`); see individual models.

pub mod entity;
pub mod migration;
