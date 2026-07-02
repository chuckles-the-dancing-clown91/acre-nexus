//! **Object storage** behind the document service (roadmap issue #17).
//!
//! Files are opaque blobs keyed by `document.storage_key`; the `document` row
//! is the only source of truth for metadata. Access is by short-lived signed
//! URL — the API hands out URLs, it never proxies S3 bytes through Rocket.
//!
//! Two backends, chosen by `STORAGE_BACKEND` (default `local`), mirroring the
//! enrichment engine's sandbox-first posture:
//!
//! * [`LocalStore`] — blobs on the local filesystem (`STORAGE_DIR`), served by
//!   the `/storage/local/…` routes. URLs carry an expiry + HMAC-SHA256
//!   signature under a key derived from `SECRETS_ENC_KEY`, so they expire and
//!   can't be forged, exactly like real presigned URLs. This is what dev/CI
//!   exercises end to end.
//! * [`S3Store`] — any S3-compatible store (AWS S3 / R2 / MinIO) via AWS
//!   Signature V4 **query presigning**, implemented directly on the `hmac` +
//!   `sha2` primitives already in the tree (no SDK dependency) and verified
//!   against the worked example in the AWS SigV4 documentation.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

type HmacSha256 = Hmac<Sha256>;

/// Default lifetime for signed upload/download URLs.
pub const SIGNED_URL_TTL_SECS: i64 = 15 * 60;

/// A signed, expiring URL plus its expiry instant (surfaced to clients).
pub struct SignedUrl {
    pub url: String,
    pub expires_at: DateTime<Utc>,
}

pub enum ObjectStore {
    Local(LocalStore),
    S3(S3Store),
}

impl ObjectStore {
    /// Build the configured store. `STORAGE_BACKEND=s3` selects [`S3Store`]
    /// (configured via `S3_BUCKET` / `S3_REGION` / `S3_ENDPOINT` /
    /// `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY`); anything else is the
    /// local filesystem store.
    pub fn from_env() -> anyhow::Result<ObjectStore> {
        match std::env::var("STORAGE_BACKEND").as_deref() {
            Ok("s3") => Ok(ObjectStore::S3(S3Store::from_env()?)),
            _ => Ok(ObjectStore::Local(LocalStore::from_env())),
        }
    }

    /// A short-lived signed URL granting one `PUT` of the blob at `key`.
    pub fn signed_put_url(&self, key: &str, ttl_secs: i64) -> anyhow::Result<SignedUrl> {
        match self {
            ObjectStore::Local(s) => Ok(s.signed_url("PUT", key, ttl_secs)),
            ObjectStore::S3(s) => s.presign("PUT", key, ttl_secs, Utc::now()),
        }
    }

    /// A short-lived signed URL granting one `GET` of the blob at `key`.
    pub fn signed_get_url(&self, key: &str, ttl_secs: i64) -> anyhow::Result<SignedUrl> {
        match self {
            ObjectStore::Local(s) => Ok(s.signed_url("GET", key, ttl_secs)),
            ObjectStore::S3(s) => s.presign("GET", key, ttl_secs, Utc::now()),
        }
    }

    /// Delete the blob at `key` (best-effort idempotent: missing is fine).
    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        match self {
            ObjectStore::Local(s) => s.delete(key),
            ObjectStore::S3(s) => s.delete(key).await,
        }
    }

    /// Store `bytes` at `key` **server-side** — for system-generated artifacts
    /// (e.g. signed PDFs) that never pass through the client upload flow.
    pub async fn put_bytes(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        match self {
            ObjectStore::Local(s) => s.put_bytes(key, bytes),
            ObjectStore::S3(s) => s.put_bytes(key, bytes).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Local filesystem store (dev/CI default)
// ---------------------------------------------------------------------------

pub struct LocalStore {
    dir: PathBuf,
    /// Public base URL of this API (the blob routes live on it).
    base_url: String,
    signing_key: Vec<u8>,
}

impl LocalStore {
    pub fn from_env() -> LocalStore {
        let dir = std::env::var("STORAGE_DIR").unwrap_or_else(|_| "./data/objects".into());
        let base_url = std::env::var("PUBLIC_API_URL")
            .unwrap_or_else(|_| "http://localhost:8000".into())
            .trim_end_matches('/')
            .to_string();
        LocalStore {
            dir: PathBuf::from(dir),
            base_url,
            signing_key: derive_signing_key(&crate::config::Config::global().secrets_key),
        }
    }

    #[cfg(test)]
    fn for_tests(dir: PathBuf) -> LocalStore {
        LocalStore {
            dir,
            base_url: "http://localhost:8000".into(),
            signing_key: vec![7u8; 32],
        }
    }

    fn signed_url(&self, method: &str, key: &str, ttl_secs: i64) -> SignedUrl {
        let expires_at = Utc::now() + chrono::Duration::seconds(ttl_secs.max(1));
        let exp = expires_at.timestamp();
        let sig = self.signature(method, key, exp);
        SignedUrl {
            url: format!(
                "{}/storage/local/{}?exp={}&sig={}",
                self.base_url,
                uri_encode(key, false),
                exp,
                sig
            ),
            expires_at,
        }
    }

    fn signature(&self, method: &str, key: &str, exp: i64) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.signing_key).expect("any key length");
        mac.update(format!("{method}\n{key}\n{exp}").as_bytes());
        mac.finalize()
            .into_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Validate an `exp`/`sig` pair presented to the blob routes. Constant-time
    /// on the signature; expired URLs are rejected outright.
    pub fn verify(&self, method: &str, key: &str, exp: i64, sig: &str) -> bool {
        if exp < Utc::now().timestamp() {
            return false;
        }
        let Some(presented) = decode_hex(sig) else {
            return false;
        };
        let mut mac = HmacSha256::new_from_slice(&self.signing_key).expect("any key length");
        mac.update(format!("{method}\n{key}\n{exp}").as_bytes());
        mac.verify_slice(&presented).is_ok()
    }

    fn blob_path(&self, key: &str) -> anyhow::Result<PathBuf> {
        // Storage keys are `{tenant_id}/{document_id}` (we mint them) — reject
        // anything that could escape the storage dir anyway.
        if key
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == ".." || seg.contains('\\'))
        {
            anyhow::bail!("invalid storage key");
        }
        Ok(self.dir.join(key))
    }

    pub fn put_bytes(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let path = self.blob_path(key)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, bytes)?;
        Ok(())
    }

    pub fn get_bytes(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let path = self.blob_path(key)?;
        match std::fs::read(&path) {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete(&self, key: &str) -> anyhow::Result<()> {
        let path = self.blob_path(key)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// The URL-signing key is derived (not identical) from the secrets key, so
/// rotating `SECRETS_ENC_KEY` also invalidates outstanding local signed URLs.
fn derive_signing_key(secrets_key: &[u8]) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(b"acre-storage-sign-v1:");
    h.update(secrets_key);
    h.finalize().to_vec()
}

/// SHA-256 of a blob, hex-encoded — the `document.checksum` format.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

// ---------------------------------------------------------------------------
// S3-compatible store (AWS SigV4 query presigning)
// ---------------------------------------------------------------------------

pub struct S3Store {
    bucket: String,
    region: String,
    /// Custom endpoint for R2/MinIO (path-style); AWS virtual-host style
    /// otherwise.
    endpoint: Option<String>,
    access_key: String,
    secret_key: String,
}

impl S3Store {
    pub fn from_env() -> anyhow::Result<S3Store> {
        let need = |name: &str| {
            std::env::var(name)
                .map_err(|_| anyhow::anyhow!("STORAGE_BACKEND=s3 requires {name} to be set"))
        };
        Ok(S3Store {
            bucket: need("S3_BUCKET")?,
            region: std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into()),
            endpoint: std::env::var("S3_ENDPOINT").ok().filter(|s| !s.is_empty()),
            access_key: need("AWS_ACCESS_KEY_ID")?,
            secret_key: need("AWS_SECRET_ACCESS_KEY")?,
        })
    }

    fn host_and_path(&self, key: &str) -> (String, String, String) {
        match &self.endpoint {
            // Path-style against a custom endpoint (MinIO / R2).
            Some(ep) => {
                let ep = ep.trim_end_matches('/');
                let host = ep
                    .trim_start_matches("https://")
                    .trim_start_matches("http://")
                    .to_string();
                let path = format!("/{}/{}", self.bucket, key);
                let url = format!("{ep}{}", uri_encode(&path, false));
                (host, path, url)
            }
            // Virtual-host style against AWS.
            None => {
                let host = if self.region == "us-east-1" {
                    format!("{}.s3.amazonaws.com", self.bucket)
                } else {
                    format!("{}.s3.{}.amazonaws.com", self.bucket, self.region)
                };
                let path = format!("/{key}");
                (
                    host.clone(),
                    path.clone(),
                    format!("https://{host}{}", uri_encode(&path, false)),
                )
            }
        }
    }

    /// Presign `method` on `key` for `ttl_secs`, per the AWS SigV4 query-string
    /// scheme with an `UNSIGNED-PAYLOAD` body.
    fn presign(
        &self,
        method: &str,
        key: &str,
        ttl_secs: i64,
        now: DateTime<Utc>,
    ) -> anyhow::Result<SignedUrl> {
        let (host, path, base_url) = self.host_and_path(key);
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let datestamp = now.format("%Y%m%d").to_string();
        let scope = format!("{datestamp}/{}/s3/aws4_request", self.region);
        let credential = format!("{}/{scope}", self.access_key);
        let expires = ttl_secs.clamp(1, 7 * 24 * 3600);

        let mut query: Vec<(String, String)> = vec![
            ("X-Amz-Algorithm".into(), "AWS4-HMAC-SHA256".into()),
            ("X-Amz-Credential".into(), credential),
            ("X-Amz-Date".into(), amz_date.clone()),
            ("X-Amz-Expires".into(), expires.to_string()),
            ("X-Amz-SignedHeaders".into(), "host".into()),
        ];
        query.sort();
        let canonical_query = query
            .iter()
            .map(|(k, v)| format!("{}={}", uri_encode(k, true), uri_encode(v, true)))
            .collect::<Vec<_>>()
            .join("&");

        let canonical_request = format!(
            "{method}\n{}\n{canonical_query}\nhost:{host}\n\nhost\nUNSIGNED-PAYLOAD",
            uri_encode(&path, false)
        );
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            sha256_hex(canonical_request.as_bytes())
        );

        let k_date = hmac_raw(
            format!("AWS4{}", self.secret_key).as_bytes(),
            datestamp.as_bytes(),
        );
        let k_region = hmac_raw(&k_date, self.region.as_bytes());
        let k_service = hmac_raw(&k_region, b"s3");
        let k_signing = hmac_raw(&k_service, b"aws4_request");
        let signature: String = hmac_raw(&k_signing, string_to_sign.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();

        Ok(SignedUrl {
            url: format!("{base_url}?{canonical_query}&X-Amz-Signature={signature}"),
            expires_at: now + chrono::Duration::seconds(expires),
        })
    }

    /// Store an object by executing a presigned `PUT` server-side.
    pub async fn put_bytes(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let signed = self.presign("PUT", key, 60, Utc::now())?;
        let client = crate::providers::client::build_http_client()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let resp = client.put(&signed.url).body(bytes.to_vec()).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("s3 put failed: HTTP {}", resp.status());
        }
        Ok(())
    }

    /// Delete an object by executing a presigned `DELETE` server-side.
    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let signed = self.presign("DELETE", key, 60, Utc::now())?;
        let client = crate::providers::client::build_http_client()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let resp = client.delete(&signed.url).send().await?;
        // 204 on delete, 404 treated as already-gone.
        if !resp.status().is_success() && resp.status().as_u16() != 404 {
            anyhow::bail!("s3 delete failed: HTTP {}", resp.status());
        }
        Ok(())
    }
}

fn hmac_raw(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// RFC 3986 percent-encoding as AWS requires it: unreserved characters pass
/// through; `/` passes through only when encoding a path.
fn uri_encode(input: &str, encode_slash: bool) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*b as char)
            }
            b'/' if !encode_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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
    use chrono::TimeZone;

    /// The worked presigned-GET example from the AWS SigV4 documentation
    /// ("Authenticating Requests: Using Query Parameters"), which pins the
    /// whole canonical-request → signing-key chain.
    #[test]
    fn sigv4_presign_matches_aws_documented_example() {
        let store = S3Store {
            bucket: "examplebucket".into(),
            region: "us-east-1".into(),
            endpoint: None,
            access_key: "AKIAIOSFODNN7EXAMPLE".into(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into(),
        };
        let now = Utc.with_ymd_and_hms(2013, 5, 24, 0, 0, 0).unwrap();
        let signed = store.presign("GET", "test.txt", 86400, now).unwrap();
        assert!(signed.url.starts_with(
            "https://examplebucket.s3.amazonaws.com/test.txt?X-Amz-Algorithm=AWS4-HMAC-SHA256"
        ));
        assert!(signed.url.ends_with(
            "X-Amz-Signature=aeeed9bbccd4d02ee5c0109b86d86835f995330da4c265957d157751f604d404"
        ));
    }

    #[test]
    fn local_store_roundtrip_and_url_verification() {
        let dir = std::env::temp_dir().join(format!("acre-storage-test-{}", uuid::Uuid::new_v4()));
        let store = LocalStore::for_tests(dir.clone());

        store.put_bytes("tenant/doc1", b"hello world").unwrap();
        assert_eq!(
            store.get_bytes("tenant/doc1").unwrap().as_deref(),
            Some(&b"hello world"[..])
        );

        let signed = store.signed_url("GET", "tenant/doc1", 60);
        let exp: i64 = signed
            .url
            .split("exp=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let sig = signed.url.split("sig=").nth(1).unwrap();
        assert!(store.verify("GET", "tenant/doc1", exp, sig));
        // Method, key, expiry and signature are all load-bearing.
        assert!(!store.verify("PUT", "tenant/doc1", exp, sig));
        assert!(!store.verify("GET", "tenant/doc2", exp, sig));
        assert!(!store.verify("GET", "tenant/doc1", exp - 10_000, sig));
        assert!(!store.verify("GET", "tenant/doc1", exp, "00ff"));

        store.delete("tenant/doc1").unwrap();
        assert!(store.get_bytes("tenant/doc1").unwrap().is_none());
        // Deleting a missing blob is fine (idempotent).
        store.delete("tenant/doc1").unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn blob_paths_cannot_escape_the_storage_dir() {
        let store = LocalStore::for_tests(std::env::temp_dir().join("acre-storage-esc"));
        assert!(store.get_bytes("../etc/passwd").is_err());
        assert!(store.get_bytes("a//b").is_err());
        assert!(store.put_bytes("./x", b"nope").is_err());
    }

    #[test]
    fn checksum_is_stable() {
        assert_eq!(
            sha256_hex(b"hello world"),
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
