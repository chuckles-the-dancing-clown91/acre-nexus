use serde::{Deserialize, Serialize};

/// A module plus its resolved enablement for the active tenant.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ModuleInfo {
    pub key: String,
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub enabled: bool,
    pub default_enabled: bool,
    pub preview: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ToggleModule {
    pub enabled: bool,
}
