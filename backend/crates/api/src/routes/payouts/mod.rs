//! **Owner payout** endpoints — compute a draft from the entity's books,
//! review it, execute it as an ACH transfer. Gated by `payout:manage`
//! (listing needs only `ledger:read`).

pub mod compute;
pub mod dto;
pub mod execute;
pub mod list;
