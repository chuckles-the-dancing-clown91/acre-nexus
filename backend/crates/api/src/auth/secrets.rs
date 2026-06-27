use rand::RngCore;
use sha2::{Digest, Sha256};

/// Generate a URL-safe random secret string of the given byte length.
pub fn random_secret(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut buf);
    // hex keeps it copy/paste-safe and fixed-width.
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// SHA-256 hash of a secret, stored instead of the raw value.
pub fn hash_secret(secret: &str) -> String {
    let mut h = Sha256::new();
    h.update(secret.as_bytes());
    format!("{:x}", h.finalize())
}
