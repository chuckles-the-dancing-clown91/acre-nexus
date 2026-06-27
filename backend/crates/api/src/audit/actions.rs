//! The audit **action taxonomy**: stable, dotted action keys.
//!
//! Keys are `resource.verb` (e.g. `property.create`). Request entries written by
//! the fairing use [`HTTP_REQUEST`]; everything else is a semantic domain event
//! written via [`super::record`]. Keeping the catalog in one place makes the set
//! of audited actions greppable and keeps the dashboard filter consistent.

/// The catch-all action for a per-request entry (the universal access log).
pub const HTTP_REQUEST: &str = "http.request";

// ---- Authentication ----
pub const AUTH_LOGIN: &str = "auth.login";
pub const AUTH_LOGOUT: &str = "auth.logout";
pub const AUTH_REFRESH: &str = "auth.refresh";
pub const AUTH_SWITCH_WORKSPACE: &str = "auth.switch_workspace";

// ---- Properties / portfolio ----
pub const PROPERTY_CREATE: &str = "property.create";
pub const PROPERTY_UPDATE: &str = "property.update";
pub const PROPERTY_ENRICH: &str = "property.enrich";
pub const LLC_CREATE: &str = "llc.create";

// ---- Leasing ----
pub const APPLICATION_SUBMIT: &str = "application.submit";
pub const APPLICATION_UPDATE: &str = "application.update";

// ---- Settings ----
pub const THEME_UPDATE: &str = "theme.update";
pub const MODULE_TOGGLE: &str = "module.toggle";

// ---- Vendor API tokens ----
pub const TOKEN_CREATE: &str = "apitoken.create";
pub const TOKEN_REVOKE: &str = "apitoken.revoke";

// ---- IAM (also referenced from the iam routes) ----
pub const USER_CREATE: &str = "user.create";
pub const ROLE_CREATE: &str = "role.create";
pub const ROLE_UPDATE: &str = "role.update";
pub const ROLE_DELETE: &str = "role.delete";
pub const PII_REVEAL: &str = "pii.reveal";
