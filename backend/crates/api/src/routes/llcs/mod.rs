//! LLC (holding-entity) endpoints — tenant-scoped.
//!
//! `list` + `create` are the basic registry (mounted by the Properties module);
//! the rest form the **onboarding** surface (mounted by the LLC Onboarding
//! module): the profile (`get`/`update`), document upload/download, branding,
//! templates, document generation, and per-tenant storage configuration.
//!
//! One handler (or one cohesive sub-resource) per file; shared request/response
//! shapes live in [`dto`] and shared internals in [`helpers`].

pub mod branding;
pub mod create;
pub mod documents;
pub mod dto;
pub mod generate;
pub mod get;
pub mod helpers;
pub mod list;
pub mod storage;
pub mod templates;
pub mod update;
