//! # Audit logging
//!
//! A fully-fledged audit trail for the platform. It captures activity at two
//! complementary levels, both persisted to the `audit_log` table and surfaced by
//! `GET /admin/audit`:
//!
//! * **Request events** — the [`AuditFairing`] records **every HTTP request**
//!   (reads included): method, path, status, latency, resolved principal, and a
//!   per-request correlation id. One wiring point ([`crate::main`] attaches the
//!   fairing) covers every current and future API automatically.
//! * **Domain events** — handlers additionally call [`record`] to log
//!   semantic state changes (`property.create`, `role.update`, `pii.reveal`, …)
//!   with rich structured `metadata`. These are the human-readable "what
//!   changed" entries.
//!
//! ## Layout
//! Each concern lives in its own small, readable file:
//! * [`actions`] — the action-key taxonomy (stable dotted strings).
//! * [`record`] — the domain-event writer.
//! * [`actor`] — resolving the principal (user / API token / public) from a request.
//! * [`request_log`] — the per-request writer used by the fairing.
//! * [`skip`] — which paths are excluded from request auditing.
//! * [`fairing`] — the Rocket fairing that ties it all together.
//!
//! Both writers are **best-effort**: a failed audit insert is logged and
//! swallowed so it can never block or fail the underlying operation.

pub mod actions;
pub mod actor;
pub mod fairing;
pub mod record;
pub mod request_log;
pub mod skip;

pub(crate) use fairing::current_request_id;
pub use fairing::AuditFairing;
pub use record::record;
