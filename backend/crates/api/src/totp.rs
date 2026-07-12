//! **TOTP** (RFC 6238) — time-based one-time passwords for authenticator-app
//! MFA (issue #63). Self-contained: RFC 4226 HOTP over HMAC-SHA1, the RFC 6238
//! time step, RFC 4648 base32 for the shared secret, and the `otpauth://` URI
//! authenticator apps import. SHA-1 is used **only** for the standard TOTP
//! construction (what Google Authenticator / Authy expect), never for security
//! hashing elsewhere.

use hmac::{Hmac, Mac};
use rand::RngCore;
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

/// The authenticator-standard parameters we issue and verify against.
pub const STEP_SECS: u64 = 30;
pub const DIGITS: u32 = 6;
/// Accept the adjacent steps too, for clock skew / entry latency.
pub const WINDOW: i64 = 1;

// ---------------------------------------------------------------------------
// base32 (RFC 4648, unpadded — the form authenticator apps accept)
// ---------------------------------------------------------------------------

const B32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// Encode bytes as unpadded uppercase base32.
pub fn base32_encode(data: &[u8]) -> String {
    let mut out = String::new();
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for &b in data {
        buffer = (buffer << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(B32[((buffer >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(B32[((buffer << (5 - bits)) & 0x1f) as usize] as char);
    }
    out
}

/// Decode base32 (case-insensitive; spaces and `=` padding are ignored).
/// Returns `None` on an invalid character.
pub fn base32_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for c in s.chars() {
        if c == '=' || c == ' ' {
            continue;
        }
        let v = match c.to_ascii_uppercase() {
            'A'..='Z' => c.to_ascii_uppercase() as u32 - 'A' as u32,
            '2'..='7' => c as u32 - '2' as u32 + 26,
            _ => return None,
        };
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// HOTP / TOTP
// ---------------------------------------------------------------------------

/// RFC 4226 HOTP: a `digits`-long one-time value for `key` at `counter`.
fn hotp(key: &[u8], counter: u64, digits: u32) -> u32 {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(&counter.to_be_bytes());
    let mac = mac.finalize().into_bytes();
    let offset = (mac[19] & 0x0f) as usize;
    let bin = ((mac[offset] as u32 & 0x7f) << 24)
        | ((mac[offset + 1] as u32) << 16)
        | ((mac[offset + 2] as u32) << 8)
        | (mac[offset + 3] as u32);
    bin % 10u32.pow(digits)
}

/// RFC 6238 TOTP value for the raw `key` at `unix_secs`. Used by the reference
/// vector tests; production verification goes through [`verify`].
#[cfg(test)]
fn totp_code(key: &[u8], unix_secs: u64, step: u64, digits: u32) -> u32 {
    hotp(key, unix_secs / step, digits)
}

/// Constant-time byte comparison (avoids leaking match progress via timing).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Verify a submitted `code` against the base32 `secret` at `unix_secs`,
/// accepting `±WINDOW` steps. Rejects malformed codes/secrets.
pub fn verify(secret_b32: &str, code: &str, unix_secs: u64) -> bool {
    let Some(key) = base32_decode(secret_b32) else {
        return false;
    };
    if key.is_empty() {
        return false;
    }
    let code = code.trim();
    if code.len() != DIGITS as usize || !code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let counter = (unix_secs / STEP_SECS) as i64;
    for w in -WINDOW..=WINDOW {
        let c = counter + w;
        if c < 0 {
            continue;
        }
        let expected = format!(
            "{:0width$}",
            hotp(&key, c as u64, DIGITS),
            width = DIGITS as usize
        );
        if ct_eq(expected.as_bytes(), code.as_bytes()) {
            return true;
        }
    }
    false
}

/// Generate a fresh 20-byte (160-bit) secret, base32-encoded for storage + the
/// `otpauth` URI.
pub fn generate_secret() -> String {
    let mut bytes = [0u8; 20];
    rand::thread_rng().fill_bytes(&mut bytes);
    base32_encode(&bytes)
}

/// The `otpauth://totp/...` URI an authenticator app imports (usually via QR).
pub fn otpauth_uri(issuer: &str, account: &str, secret_b32: &str) -> String {
    // The label is `issuer:account`; the ':' separator stays literal, each side
    // is percent-encoded on its own.
    let label = format!("{}:{}", percent_encode(issuer), percent_encode(account));
    let iss = percent_encode(issuer);
    format!(
        "otpauth://totp/{label}?secret={secret_b32}&issuer={iss}\
         &algorithm=SHA1&digits={DIGITS}&period={STEP_SECS}"
    )
}

/// Minimal percent-encoding for the `otpauth` label/issuer (keep unreserved
/// chars, escape everything else).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_known_vectors_and_roundtrip() {
        // RFC 4648 test vectors (unpadded).
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32_encode(b"f"), "MY");
        assert_eq!(base32_decode("mzxw6ytboi").unwrap(), b"foobar");
        for s in [b"Hello, TOTP!".as_slice(), &[0u8, 255, 42, 7, 200]] {
            assert_eq!(base32_decode(&base32_encode(s)).unwrap(), s);
        }
        assert!(base32_decode("not!base32").is_none());
    }

    #[test]
    fn rfc6238_sha1_reference_vectors() {
        // RFC 6238 Appendix B, SHA-1, 8 digits, secret = ASCII "12345678..0".
        let secret = b"12345678901234567890";
        let cases = [
            (59u64, 94287082u32),
            (1111111109, 7081804),
            (1111111111, 14050471),
            (1234567890, 89005924),
            (2000000000, 69279037),
        ];
        for (t, expected) in cases {
            assert_eq!(totp_code(secret, t, STEP_SECS, 8), expected, "T={t}");
        }
    }

    #[test]
    fn verify_accepts_current_and_adjacent_windows() {
        let secret = generate_secret();
        let key = base32_decode(&secret).unwrap();
        let now = 1_700_000_000u64;
        let code = format!("{:06}", totp_code(&key, now, STEP_SECS, DIGITS));
        assert!(verify(&secret, &code, now));
        // One step earlier/later is still accepted (skew tolerance)…
        assert!(verify(&secret, &code, now + STEP_SECS));
        assert!(verify(&secret, &code, now.saturating_sub(STEP_SECS)));
        // …but two steps away, or a wrong/malformed code, is not.
        assert!(!verify(&secret, &code, now + 2 * STEP_SECS));
        assert!(!verify(&secret, "000000", now.wrapping_add(9_999)));
        assert!(!verify(&secret, "12345", now)); // too short
        assert!(!verify(&secret, "abcdef", now)); // non-numeric
    }

    #[test]
    fn otpauth_uri_shape() {
        let uri = otpauth_uri("Acre Nexus", "jordan@northwind.com", "ABCDEF");
        assert!(uri.starts_with("otpauth://totp/Acre%20Nexus:jordan%40northwind.com?"));
        assert!(uri.contains("secret=ABCDEF"));
        assert!(uri.contains("algorithm=SHA1"));
        assert!(uri.contains("digits=6"));
        assert!(uri.contains("period=30"));
    }
}
