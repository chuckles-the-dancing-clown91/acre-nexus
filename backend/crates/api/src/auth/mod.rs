//! Authentication primitives: password hashing, JWT access tokens, opaque
//! refresh/secret tokens, and the `AuthUser` request guard.

pub mod guard;
pub mod jwt;
pub mod passwords;
pub mod secrets;

pub use guard::AuthUser;
pub(crate) use jwt::decode_access_token;
pub use jwt::issue_access_token;
pub use passwords::{hash_password, verify_password};
pub use secrets::{hash_secret, random_secret};
