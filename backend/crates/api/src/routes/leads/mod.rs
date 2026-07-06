//! **CRM lead** endpoints (the #46 seed, landed with #62's inbound email).
//! Reading needs `application:read`; working a lead needs `application:write`
//! — leads are the front of the same funnel applications ride.

pub mod dto;
pub mod list;
pub mod update;
