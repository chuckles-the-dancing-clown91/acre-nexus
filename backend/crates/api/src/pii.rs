//! Field-level encryption for sensitive PII (SSN, government-ID numbers).
//!
//! Values are sealed with **AES-256-GCM** (authenticated encryption) under a
//! 32-byte key from configuration ([`crate::config::Config::pii_key`]). We store
//! the base64 ciphertext + a fresh random 96-bit nonce per value, plus the last
//! four digits in clear for display. Decryption is an explicit, permission-gated
//! action (`profile:read_pii`) — see the admin profile routes.
//!
//! Production note: back the key with a KMS/HSM and rotate it; this module keeps
//! the boundary small so swapping the key source is a one-line change.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;

/// A sealed value: base64 ciphertext + base64 nonce.
pub struct Sealed {
    pub ciphertext: String,
    pub nonce: String,
}

/// Encrypt `plaintext` with the 32-byte `key`. Returns ciphertext + nonce.
pub fn encrypt(key: &[u8], plaintext: &str) -> anyhow::Result<Sealed> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("pii encrypt failed: {e}"))?;
    Ok(Sealed {
        ciphertext: B64.encode(ct),
        nonce: B64.encode(nonce_bytes),
    })
}

/// Decrypt a previously [`encrypt`]ed value.
pub fn decrypt(key: &[u8], ciphertext_b64: &str, nonce_b64: &str) -> anyhow::Result<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let ct = B64.decode(ciphertext_b64)?;
    let nonce_bytes = B64.decode(nonce_b64)?;
    if nonce_bytes.len() != 12 {
        anyhow::bail!("invalid nonce length");
    }
    let nonce = Nonce::from_slice(&nonce_bytes);
    let pt = cipher
        .decrypt(nonce, ct.as_ref())
        .map_err(|e| anyhow::anyhow!("pii decrypt failed: {e}"))?;
    Ok(String::from_utf8(pt)?)
}

/// The last four characters of a value (for masked display). Shorter inputs are
/// returned as-is.
pub fn last4(value: &str) -> String {
    let digits: String = value.chars().filter(|c| c.is_alphanumeric()).collect();
    let n = digits.chars().count();
    if n <= 4 {
        digits
    } else {
        digits.chars().skip(n - 4).collect()
    }
}
