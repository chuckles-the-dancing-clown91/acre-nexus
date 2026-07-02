//! Inbound **webhook signature verification** (the receiving half of #16).
//!
//! Providers sign the raw request body with HMAC-SHA256 over a shared signing
//! secret (stored per tenant in the secrets store under
//! `webhook.<provider>.secret`). Verification is constant-time via
//! [`hmac::Mac::verify_slice`] — the codebase's first real MAC comparison; the
//! bearer-token path (`tokens::principal`) only ever compared hashes by DB
//! lookup.
//!
//! Signature header format: `X-Acre-Signature: sha256=<hex hmac of raw body>`.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// The name of the secrets-store key holding `provider`'s signing secret.
pub fn secret_key_name(provider: &str) -> String {
    format!("webhook.{provider}.secret")
}

/// Compute the signature header value for `body` — exercised by the tests
/// below; outbound webhook delivery (#68) is its first production caller.
#[allow(dead_code)]
pub fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    let out = mac.finalize().into_bytes();
    let hex: String = out.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256={hex}")
}

/// Verify a presented `X-Acre-Signature` header against the raw body. The
/// comparison is constant-time; any parse failure is a plain rejection.
pub fn verify(secret: &str, body: &[u8], header: &str) -> bool {
    let Some(hex) = header.trim().strip_prefix("sha256=") else {
        return false;
    };
    let Some(presented) = decode_hex(hex) else {
        return false;
    };
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(body);
    mac.verify_slice(&presented).is_ok()
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_then_verify_roundtrips() {
        let sig = sign("whsec_test", b"{\"event\":\"ping\"}");
        assert!(sig.starts_with("sha256="));
        assert!(verify("whsec_test", b"{\"event\":\"ping\"}", &sig));
    }

    #[test]
    fn verify_rejects_tampering() {
        let sig = sign("whsec_test", b"{\"event\":\"ping\"}");
        // Body changed.
        assert!(!verify("whsec_test", b"{\"event\":\"pong\"}", &sig));
        // Wrong secret.
        assert!(!verify("whsec_other", b"{\"event\":\"ping\"}", &sig));
        // Malformed headers.
        assert!(!verify("whsec_test", b"{}", "md5=abc"));
        assert!(!verify("whsec_test", b"{}", "sha256=zz"));
        assert!(!verify("whsec_test", b"{}", ""));
    }

    #[test]
    fn secret_key_name_is_stable() {
        assert_eq!(secret_key_name("stripe"), "webhook.stripe.secret");
    }
}
