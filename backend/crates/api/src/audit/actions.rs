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
/// A background enrichment source finished writing property data (or gave up
/// retrying). Distinct from [`PROPERTY_ENRICH`], which only logs the request
/// that *enqueued* the job — this is the actual data mutation.
pub const PROPERTY_ENRICHMENT_RUN: &str = "property.enrichment_run";
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

// ---- E-signature envelopes (Phase 2) ----
pub const ESIGN_SEND: &str = "esign.send";
pub const ESIGN_VIEW: &str = "esign.view";
pub const ESIGN_SIGN: &str = "esign.sign";
pub const ESIGN_DECLINE: &str = "esign.decline";
pub const ESIGN_REMIND: &str = "esign.remind";
pub const ESIGN_COMPLETE: &str = "esign.complete";
pub const ESIGN_VOID: &str = "esign.void";

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
/// A background screening finished and its verdict landed on the application
/// (actor is `None`: the pipeline wrote it, not a person).
pub const APPLICATION_SCREENED: &str = "application.screened";
/// A screening report was ordered from the consumer-reporting provider
/// (consent timestamp recorded in metadata — FCRA §604(b)).
pub const SCREENING_ORDERED: &str = "screening.ordered";
/// The provider's report completed and the policy verdict was recorded.
pub const SCREENING_COMPLETED: &str = "screening.completed";
/// An FCRA §615(a) adverse-action notice was sent (and filed) for a declined
/// application.
pub const ADVERSE_ACTION: &str = "application.adverse_action";
pub const LISTING_CREATE: &str = "listing.create";
pub const LISTING_UPDATE: &str = "listing.update";
/// The pipeline moved a listing's status automatically (conversion → Pending,
/// activation → Leased, declined envelope → Available) — distinct from a
/// staff-driven [`LISTING_UPDATE`].
pub const LISTING_SYNC: &str = "listing.sync";
/// A lease flipped to `active` because its document was signed.
pub const LEASE_ACTIVATE: &str = "lease.activate";

// ---- Settings ----
pub const THEME_UPDATE: &str = "theme.update";
pub const MODULE_TOGGLE: &str = "module.toggle";

// ---- Vendor API tokens ----
pub const TOKEN_CREATE: &str = "apitoken.create";
pub const TOKEN_REVOKE: &str = "apitoken.revoke";

// ---- Integrations: secrets, documents, notifications, providers, webhooks ----
// Secrets log the *fact* and the key name, never the value (docs/AUDIT.md).
pub const SECRET_SET: &str = "secret.set";
pub const SECRET_ROTATE: &str = "secret.rotate";
pub const SECRET_DELETE: &str = "secret.delete";
pub const DOCUMENT_UPLOAD: &str = "document.upload";
/// A tokenized blob PUT landed and finalized the document row (size,
/// checksum, `stored`) — the completion of the upload that
/// [`DOCUMENT_UPLOAD`] initiated.
pub const DOCUMENT_STORED: &str = "document.stored";
/// The fact a download URL was issued — not the content (same discipline as
/// `pii.reveal`).
pub const DOCUMENT_DOWNLOAD: &str = "document.download";
/// Filing metadata changed (category, wet-ink flag, physical storage location).
pub const DOCUMENT_UPDATE: &str = "document.update";
pub const DOCUMENT_DELETE: &str = "document.delete";
pub const NOTIFICATION_SEND: &str = "notification.send";
/// Staff triggered a test delivery (provider test or own-device push test).
pub const NOTIFICATION_TEST: &str = "notification.test";
/// Inbox entries marked read (self-service; count in metadata).
pub const NOTIFICATION_READ: &str = "notification.read";
/// One event fanned out to staff (in-app + push + chat) — recipients counted
/// in metadata; individual sends audit separately as `notification.send`.
pub const NOTIFICATION_BROADCAST: &str = "notification.broadcast";
pub const NOTIFICATION_PROVIDER_CREATE: &str = "notification_provider.create";
pub const NOTIFICATION_PROVIDER_UPDATE: &str = "notification_provider.update";
pub const NOTIFICATION_PROVIDER_DELETE: &str = "notification_provider.delete";
// Message-template settings: edits log the key, never the rendered content.
pub const NOTIFICATION_TEMPLATE_UPDATE: &str = "notification_template.update";
pub const NOTIFICATION_TEMPLATE_RESET: &str = "notification_template.reset";
pub const NOTIFICATION_TEMPLATE_IMPORT: &str = "notification_template.import";
pub const PUSH_SUBSCRIBE: &str = "push.subscribe";
pub const PUSH_UNSUBSCRIBE: &str = "push.unsubscribe";
/// One outbound provider invocation (simulated or live) by the job runner.
pub const PROVIDER_CALL: &str = "provider.call";
/// A signature-verified inbound webhook was accepted and enqueued.
pub const WEBHOOK_RECEIVED: &str = "webhook.received";

// ---- Accounting & payments (Phase 3) ----
/// A balanced double-entry transaction landed on an entity's books (actor is
/// `None` when the pipeline posted it, the user for manual journal entries).
pub const LEDGER_POST: &str = "ledger.post";
pub const LEDGER_ACCOUNT_CREATE: &str = "ledger_account.create";
/// A payment was initiated (portal "pay now", autopay, or staff collect).
pub const PAYMENT_CREATE: &str = "payment.create";
/// A payment reached a terminal state (paid/failed) — written by the pipeline
/// (simulated settlement or a processor webhook), not a person.
pub const PAYMENT_SETTLE: &str = "payment.settle";
pub const PAYMENT_METHOD_ADD: &str = "payment_method.add";
pub const PAYMENT_METHOD_REMOVE: &str = "payment_method.remove";
pub const AUTOPAY_ENROLL: &str = "autopay.enroll";
pub const AUTOPAY_CANCEL: &str = "autopay.cancel";
/// The billing cycle assessed a late fee against an overdue receivable.
pub const LATE_FEE_APPLY: &str = "late_fee.apply";
pub const PAYOUT_CREATE: &str = "payout.create";
pub const PAYOUT_EXECUTE: &str = "payout.execute";
/// A payout reached a terminal state (paid/failed).
pub const PAYOUT_SETTLE: &str = "payout.settle";
/// A bank account was linked for feeds (Plaid or simulated).
pub const BANK_ACCOUNT_LINK: &str = "bank_account.link";
/// One bank-feed sync pulled transactions for a linked account.
pub const BANK_FEED_SYNC: &str = "bank_feed.sync";
/// A bank transaction was matched to (or unmatched from) a payment.
pub const BANK_TXN_MATCH: &str = "bank_txn.match";

// ---- IAM (also referenced from the iam routes) ----
pub const USER_CREATE: &str = "user.create";
pub const USER_UPDATE: &str = "user.update";
pub const ROLE_CREATE: &str = "role.create";
pub const ROLE_UPDATE: &str = "role.update";
pub const ROLE_DELETE: &str = "role.delete";
pub const ROLE_ASSIGN: &str = "role.assign";
pub const ROLE_REVOKE: &str = "role.revoke";
pub const MEMBERSHIP_ADD: &str = "membership.add";
pub const MEMBERSHIP_REMOVE: &str = "membership.remove";
pub const PROFILE_WRITE: &str = "profile.write";
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
pub const ASSIGNMENT_CREATE: &str = "assignment.create";
pub const ASSIGNMENT_REMOVE: &str = "assignment.remove";
pub const SETTING_UPDATE: &str = "setting.update";
pub const APPLICATION_ADVANCE: &str = "application.advance";
pub const APPLICATION_REUSE: &str = "application.reuse";
