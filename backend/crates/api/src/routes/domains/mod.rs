//! White-label **domain & routing** endpoints (§7): map hosts to a tenant + an
//! audience (`admin` / `owner` / `renter`), verify custom domains, and resolve an
//! inbound host for the unauthenticated routing layer.

pub mod create;
pub mod delete;
pub mod dto;
pub mod list;
pub mod resolve;
pub mod verify;
pub mod verify_email;
