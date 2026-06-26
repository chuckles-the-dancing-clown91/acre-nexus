use crate::auth::{hash_secret, random_secret};

/// Prefix that distinguishes a vendor key from a JWT in the `Authorization` header.
pub const TOKEN_PREFIX: &str = "acre_live_";

/// A freshly minted token — the raw secret is returned to the caller exactly once.
pub struct MintedToken {
    pub raw: String,
    pub prefix: String,
    pub hash: String,
}

/// Mint a new vendor token secret + its stored hash.
pub fn mint() -> MintedToken {
    let secret = random_secret(24);
    let raw = format!("{TOKEN_PREFIX}{secret}");
    // Visible prefix shown in dashboards for identification.
    let prefix = format!("{TOKEN_PREFIX}{}", &secret[..6]);
    let hash = hash_secret(&raw);
    MintedToken { raw, prefix, hash }
}
