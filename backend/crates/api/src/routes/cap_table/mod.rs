//! **Cap table** endpoints — the ownership structure of a legal entity (§6): the
//! owners/investors (`owner`) and their stakes (`entity_ownership`). The firm
//! itself can be an owner; external investors are owners too.

pub mod add;
pub mod dto;
pub mod list;
