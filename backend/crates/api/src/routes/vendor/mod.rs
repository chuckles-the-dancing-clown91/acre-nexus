//! Versioned **vendor API** (`/api/v1`). Authenticated with scoped API tokens
//! (not JWTs), this is the surface sold to third-party vendors so Acre services
//! can be leveraged à la carte. Each endpoint requires a specific token scope.

pub mod dto;
pub mod listings;
pub mod properties;
