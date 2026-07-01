//! `/settings` — read and edit the per-tenant [`crate::settings`] catalog.
//! Gated by `tenant:manage` (workspace administration).

pub mod list;
pub mod set;

use serde::{Deserialize, Serialize};

/// A setting merged with its catalog metadata and the tenant's effective value.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SettingView {
    pub key: String,
    pub label: String,
    pub description: String,
    pub group: String,
    /// `bool` | `int` | `text`.
    pub kind: String,
    /// The effective value for this tenant (override or catalog default).
    pub value: serde_json::Value,
    /// The catalog default, for "reset" affordances.
    pub default: serde_json::Value,
}

/// Body for `PUT /settings/<key>`.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct SetSettingReq {
    pub value: serde_json::Value,
}
