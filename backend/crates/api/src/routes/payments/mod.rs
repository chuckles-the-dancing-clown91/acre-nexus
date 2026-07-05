//! **Payments** endpoints — staff visibility over rent collection
//! (`payment:read`/`payment:manage`) and the renter portal's self-service
//! `/my/*` payment surface: view the lease + balance, save tokenized
//! methods, pay due items, and enroll in autopay.

pub mod dto;
pub mod list;
pub mod methods;
pub mod portal;
