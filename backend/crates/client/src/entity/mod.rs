//! SeaORM models for the **client** domain (`acre_client` database).
//!
//! Cross-domain references (e.g. `application.listing_id` → a property listing,
//! `counterparty_note.author_user_id` → a user) are plain `Uuid` columns
//! enforced by the application layer, never DB foreign keys.

pub mod application;
pub mod counterparty;
pub mod counterparty_note;
