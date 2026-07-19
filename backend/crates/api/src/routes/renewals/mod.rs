//! **Lease renewal** endpoints (issue #44) — the ongoing-tenancy motion:
//! propose renewed terms (a rent increase + extended end date), generate an
//! addendum, send it for e-signature, and on completion apply the new terms to
//! the lease. Reading needs `lease:read`; proposing/sending/cancelling needs
//! `lease:manage`. Owned by the Lease Builder & Tenancy module, alongside lease
//! documents + e-signature.

pub mod cancel;
pub mod dto;
pub mod list;
pub mod propose;
pub mod send;
