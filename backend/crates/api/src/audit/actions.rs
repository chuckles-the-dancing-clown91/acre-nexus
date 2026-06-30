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
pub const PROPERTY_ONBOARD: &str = "property.onboard";
pub const LLC_CREATE: &str = "llc.create";

// ---- Investing: entities, financing, workflow ----
pub const ENTITY_CREATE: &str = "entity.create";
pub const ENTITY_UPDATE: &str = "entity.update";
pub const ENTITY_NOTE_ADD: &str = "entity.note_add";
pub const MORTGAGE_CREATE: &str = "mortgage.create";
pub const MORTGAGE_UPDATE: &str = "mortgage.update";
pub const MORTGAGE_DELETE: &str = "mortgage.delete";
pub const WORKFLOW_ADVANCE: &str = "workflow.advance";

// ---- Rentals ----
pub const UNIT_CREATE: &str = "unit.create";
pub const UNIT_UPDATE: &str = "unit.update";
pub const LEASE_CREATE: &str = "lease.create";
pub const LEASE_UPDATE: &str = "lease.update";
pub const LEASE_PAYMENT_RECORD: &str = "lease.payment_record";

// ---- Leasing lifecycle: fees, charges, vehicles, documents, conversion ----
pub const FEE_SCHEDULE_CREATE: &str = "fee_schedule.create";
pub const FEE_SCHEDULE_UPDATE: &str = "fee_schedule.update";
pub const FEE_SCHEDULE_DELETE: &str = "fee_schedule.delete";
pub const LEASE_CHARGE_ADD: &str = "lease_charge.add";
pub const LEASE_CHARGE_REMOVE: &str = "lease_charge.remove";
pub const LEASE_FEES_APPLY: &str = "lease.apply_fees";
pub const LEASE_DOC_GENERATE: &str = "lease_document.generate";
pub const LEASE_DOC_SIGN: &str = "lease_document.sign";
pub const VEHICLE_CREATE: &str = "vehicle.create";
pub const VEHICLE_UPDATE: &str = "vehicle.update";
pub const VEHICLE_DELETE: &str = "vehicle.delete";
pub const APPLICATION_CONVERT: &str = "application.convert";

// ---- Maintenance ----
pub const TICKET_CREATE: &str = "ticket.create";
pub const TICKET_UPDATE: &str = "ticket.update";
pub const TICKET_COMMENT_ADD: &str = "ticket.comment_add";

// ---- Title: ownership + liens ----
pub const OWNERSHIP_CREATE: &str = "ownership.create";
pub const OWNERSHIP_UPDATE: &str = "ownership.update";
pub const OWNERSHIP_DELETE: &str = "ownership.delete";
pub const LIEN_CREATE: &str = "lien.create";
pub const LIEN_UPDATE: &str = "lien.update";
pub const LIEN_DELETE: &str = "lien.delete";

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
pub const ROLE_ASSIGN: &str = "role.assign";
pub const PII_REVEAL: &str = "pii.reveal";

// ---- Tenancy spec: provisioning, platform plane, routing, multi-entity ----
pub const TENANT_PROVISION: &str = "tenant.provision";
pub const IMPERSONATION_START: &str = "impersonation.start";
pub const IMPERSONATION_REVOKE: &str = "impersonation.revoke";
pub const DOMAIN_CREATE: &str = "domain.create";
pub const DOMAIN_VERIFY: &str = "domain.verify";
pub const DOMAIN_DELETE: &str = "domain.delete";
pub const PORTFOLIO_CREATE: &str = "portfolio.create";
pub const OWNER_CREATE: &str = "owner.create";
pub const ENTITY_OWNERSHIP_ADD: &str = "entity_ownership.add";
pub const BANK_ACCOUNT_CREATE: &str = "bank_account.create";
pub const ONBOARDING_ADVANCE: &str = "onboarding.advance";
