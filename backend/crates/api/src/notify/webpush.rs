//! **Web Push** delivery — the browser push channel, implemented directly on
//! the standards rather than an SDK:
//!
//! * payload encryption per **RFC 8291** (`aes128gcm` content coding: ECDH on
//!   P-256 + HKDF-SHA256 + AES-128-GCM), pinned by the RFC's Appendix A test
//!   vector below;
//! * request signing per **RFC 8292 (VAPID)** — an ES256 JWT over the push
//!   service's origin.
//!
//! The platform's VAPID keypair is generated on first use and kept in the
//! secrets vault as a platform-wide secret, so no operator key ceremony is
//! needed; the public key is served to browsers via
//! `GET /notifications/vapid_key`.

use crate::providers::{client, err, Provider, ProviderCtx, ProviderError};
use crate::secrets;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use base64::Engine;
use hkdf::Hkdf;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{PublicKey, SecretKey};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, ModelTrait, QueryFilter};
use serde::Serialize;
use sha2::Sha256;
use uuid::Uuid;

/// Vault key (platform-wide) holding the base64url P-256 VAPID private scalar.
pub const VAPID_SECRET_KEY_NAME: &str = "webpush.vapid_private_key";

/// Load the platform VAPID signing key, generating and vaulting one on first
/// use.
pub async fn vapid_signing_key(db: &impl ConnectionTrait) -> anyhow::Result<SigningKey> {
    if let Some(b64) = secrets::reveal(db, None, VAPID_SECRET_KEY_NAME).await? {
        let bytes = B64URL
            .decode(b64.trim())
            .map_err(|e| anyhow::anyhow!("stored VAPID key is not base64url: {e}"))?;
        return SigningKey::from_slice(&bytes)
            .map_err(|e| anyhow::anyhow!("stored VAPID key is invalid: {e}"));
    }
    let sk = SigningKey::random(&mut rand::rngs::OsRng);
    let encoded = B64URL.encode(sk.to_bytes());
    secrets::store(db, None, VAPID_SECRET_KEY_NAME, &encoded, None).await?;
    tracing::info!("generated a new platform VAPID keypair (vaulted)");
    Ok(sk)
}

/// The base64url uncompressed public point browsers pass as
/// `applicationServerKey`.
pub fn vapid_public_key_b64(sk: &SigningKey) -> String {
    B64URL.encode(VerifyingKey::from(sk).to_encoded_point(false).as_bytes())
}

/// `Authorization: vapid t=<jwt>, k=<pub>` for one push-service endpoint.
fn vapid_auth_header(sk: &SigningKey, endpoint: &str) -> Result<String, ProviderError> {
    // aud is the push service origin (scheme://host).
    let rest = endpoint
        .strip_prefix("https://")
        .ok_or_else(|| err("push endpoint must be https"))?;
    let host = rest.split('/').next().unwrap_or_default();
    if host.is_empty() {
        return Err(err("push endpoint has no host"));
    }
    let aud = format!("https://{host}");
    let sub =
        std::env::var("VAPID_SUBJECT").unwrap_or_else(|_| "mailto:ops@acrenexus.example".into());
    let exp = chrono::Utc::now().timestamp() + 12 * 3600;

    let header = B64URL.encode(br#"{"typ":"JWT","alg":"ES256"}"#);
    let payload = B64URL.encode(
        serde_json::json!({ "aud": aud, "exp": exp, "sub": sub })
            .to_string()
            .as_bytes(),
    );
    let signing_input = format!("{header}.{payload}");
    let sig: Signature = sk.sign(signing_input.as_bytes());
    let jwt = format!("{signing_input}.{}", B64URL.encode(sig.to_bytes()));
    Ok(format!("vapid t={jwt}, k={}", vapid_public_key_b64(sk)))
}

/// Encrypt `plaintext` for a subscription per RFC 8291 (`aes128gcm` coding).
/// `salt` and the ephemeral application-server key are injected so the test
/// vector can pin the whole derivation; production callers pass fresh random
/// values.
fn encrypt_payload(
    ua_public_b64: &str,
    auth_b64: &str,
    plaintext: &[u8],
    salt: &[u8; 16],
    as_secret: &SecretKey,
) -> Result<Vec<u8>, ProviderError> {
    let ua_public_bytes = B64URL
        .decode(ua_public_b64.trim())
        .map_err(|e| err(format!("bad p256dh key: {e}")))?;
    let ua_public = PublicKey::from_sec1_bytes(&ua_public_bytes)
        .map_err(|e| err(format!("bad p256dh key: {e}")))?;
    let auth = B64URL
        .decode(auth_b64.trim())
        .map_err(|e| err(format!("bad auth secret: {e}")))?;
    if auth.len() != 16 {
        return Err(err("auth secret must be 16 bytes"));
    }

    let shared = p256::ecdh::diffie_hellman(as_secret.to_nonzero_scalar(), ua_public.as_affine());
    let as_public = as_secret.public_key().to_encoded_point(false);
    let ua_point = ua_public.to_encoded_point(false);

    // IKM = HKDF(salt=auth, ikm=ecdh, info="WebPush: info"||0||ua_pub||as_pub)
    let mut key_info = Vec::with_capacity(14 + 65 + 65);
    key_info.extend_from_slice(b"WebPush: info\0");
    key_info.extend_from_slice(ua_point.as_bytes());
    key_info.extend_from_slice(as_public.as_bytes());
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(&auth), shared.raw_secret_bytes())
        .expand(&key_info, &mut ikm)
        .map_err(|_| err("hkdf expand (ikm) failed"))?;

    // RFC 8188: CEK + NONCE from HKDF(salt, IKM).
    let hk = Hkdf::<Sha256>::new(Some(salt), &ikm);
    let mut cek = [0u8; 16];
    hk.expand(b"Content-Encoding: aes128gcm\0", &mut cek)
        .map_err(|_| err("hkdf expand (cek) failed"))?;
    let mut nonce = [0u8; 12];
    hk.expand(b"Content-Encoding: nonce\0", &mut nonce)
        .map_err(|_| err("hkdf expand (nonce) failed"))?;

    // Single (last) record: plaintext || 0x02 padding delimiter.
    let mut record = plaintext.to_vec();
    record.push(0x02);
    let cipher = Aes128Gcm::new_from_slice(&cek).map_err(|_| err("bad content-encryption key"))?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), record.as_slice())
        .map_err(|_| err("payload encryption failed"))?;

    // aes128gcm header: salt(16) || record-size(4, BE) || keyid-len(1) || keyid(as_public).
    let mut body = Vec::with_capacity(16 + 4 + 1 + 65 + ciphertext.len());
    body.extend_from_slice(salt);
    body.extend_from_slice(&4096u32.to_be_bytes());
    body.push(as_public.as_bytes().len() as u8);
    body.extend_from_slice(as_public.as_bytes());
    body.extend_from_slice(&ciphertext);
    Ok(body)
}

/// Deliver one encrypted payload to one subscription. Returns the push
/// service's HTTP status (404/410 mean the subscription is gone).
async fn deliver(
    sk: &SigningKey,
    sub: &entity::push_subscription::Model,
    payload: &str,
) -> Result<u16, ProviderError> {
    let mut salt = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut salt);
    let as_secret = SecretKey::random(&mut rand::rngs::OsRng);
    let body = encrypt_payload(
        &sub.p256dh,
        &sub.auth,
        payload.as_bytes(),
        &salt,
        &as_secret,
    )?;

    let http = client::build_http_client()?;
    let resp = http
        .post(&sub.endpoint)
        .header("TTL", "86400")
        .header("Content-Encoding", "aes128gcm")
        .header("Urgency", "normal")
        .header("Authorization", vapid_auth_header(sk, &sub.endpoint)?)
        .body(body)
        .send()
        .await
        .map_err(|e| err(format!("push delivery failed: {e}")))?;
    Ok(resp.status().as_u16())
}

// ---------------------------------------------------------------------------
// The push provider (rides the #16 trait like every other channel)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct PushRequest {
    pub user_id: Uuid,
    pub title: String,
    pub body: String,
}

pub struct PushDelivery;

#[async_trait::async_trait]
impl Provider for PushDelivery {
    type Request = PushRequest;
    type Response = super::delivery::MessageResponse;

    fn key(&self) -> &'static str {
        "push"
    }

    async fn call<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let subs = subscriptions(ctx, req.user_id).await?;
        if subs.is_empty() {
            return Ok(super::delivery::MessageResponse {
                provider_message_id: "webpush:no-subscriptions".into(),
            });
        }
        let sk = vapid_signing_key(ctx.db)
            .await
            .map_err(|e| err(format!("vapid key unavailable: {e}")))?;
        let payload = serde_json::json!({
            "title": req.title,
            "body": req.body,
            "url": "/console/notifications",
        })
        .to_string();

        let total = subs.len();
        let mut delivered = 0usize;
        let mut gone = 0usize;
        let mut last_error: Option<ProviderError> = None;
        for sub in subs {
            match deliver(&sk, &sub, &payload).await {
                Ok(status) if (200..300).contains(&(status as i32)) => delivered += 1,
                // The push service says this subscription no longer exists —
                // prune it so we stop trying.
                Ok(404) | Ok(410) => {
                    gone += 1;
                    if let Err(e) = sub.delete(ctx.db).await {
                        tracing::warn!("failed to prune dead push subscription: {e}");
                    }
                }
                Ok(status) => last_error = Some(err(format!("push service returned {status}"))),
                Err(e) => last_error = Some(e),
            }
        }
        // Only a total failure is an error (→ retry); partial delivery counts.
        if delivered == 0 && gone < total {
            if let Some(e) = last_error {
                return Err(e);
            }
        }
        Ok(super::delivery::MessageResponse {
            provider_message_id: format!("webpush:{delivered}/{total}"),
        })
    }

    async fn simulate<C: ConnectionTrait + Sync>(
        &self,
        ctx: &ProviderCtx<'_, C>,
        req: &Self::Request,
    ) -> Result<Self::Response, ProviderError> {
        let subs = subscriptions(ctx, req.user_id).await?;
        tracing::info!(
            user = %req.user_id,
            subscriptions = subs.len(),
            "simulated web push: {}",
            req.title
        );
        Ok(super::delivery::MessageResponse {
            provider_message_id: format!("sim-push-{}", Uuid::new_v4().simple()),
        })
    }
}

async fn subscriptions<C: ConnectionTrait + Sync>(
    ctx: &ProviderCtx<'_, C>,
    user_id: Uuid,
) -> Result<Vec<entity::push_subscription::Model>, ProviderError> {
    entity::prelude::PushSubscription::find()
        .filter(entity::push_subscription::Column::TenantId.eq(ctx.tenant_id))
        .filter(entity::push_subscription::Column::UserId.eq(user_id))
        .all(ctx.db)
        .await
        .map_err(|e| err(format!("db error loading push subscriptions: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::Verifier;

    /// RFC 8291 Appendix A — the complete worked example. Pins the ECDH →
    /// HKDF → AES-128-GCM chain and the aes128gcm framing byte for byte.
    #[test]
    fn rfc8291_appendix_a_test_vector() {
        let plaintext = b"When I grow up, I want to be a watermelon";
        let ua_public = "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
        let auth = "BTBZMqHH6r4Tts7J_aSIgg";
        let as_private = B64URL
            .decode("yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw")
            .unwrap();
        let salt: [u8; 16] = B64URL
            .decode("DGv6ra1nlYgDCS1FRnbzlw")
            .unwrap()
            .try_into()
            .unwrap();
        // RFC 8291 §5 / Appendix A, verbatim (line wraps joined).
        let expected = B64URL
            .decode(
                "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27ml\
                 mlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPT\
                 pK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN",
            )
            .unwrap();

        let as_secret = SecretKey::from_slice(&as_private).unwrap();
        let body = encrypt_payload(ua_public, auth, plaintext, &salt, &as_secret).unwrap();
        assert_eq!(body, expected);
    }

    #[test]
    fn vapid_header_is_a_verifiable_es256_jwt() {
        let sk = SigningKey::random(&mut rand::rngs::OsRng);
        let header = vapid_auth_header(&sk, "https://fcm.googleapis.com/fcm/send/abc123").unwrap();
        let token = header
            .strip_prefix("vapid t=")
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);

        // Claims carry the push-service origin as `aud`.
        let claims: serde_json::Value =
            serde_json::from_slice(&B64URL.decode(parts[1]).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://fcm.googleapis.com");
        assert!(claims["exp"].as_i64().unwrap() > chrono::Utc::now().timestamp());

        // The signature verifies under the advertised public key.
        let sig = Signature::from_slice(&B64URL.decode(parts[2]).unwrap()).unwrap();
        VerifyingKey::from(&sk)
            .verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &sig)
            .unwrap();

        // And the k= parameter is the same key browsers subscribe with.
        assert!(header.ends_with(&format!("k={}", vapid_public_key_b64(&sk))));
    }

    #[test]
    fn rejects_malformed_subscriptions() {
        let sk = SecretKey::random(&mut rand::rngs::OsRng);
        let salt = [0u8; 16];
        assert!(
            encrypt_payload("not-base64!!", "BTBZMqHH6r4Tts7J_aSIgg", b"x", &salt, &sk).is_err()
        );
        assert!(encrypt_payload(
            "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
            "dG9vc2hvcnQ",
            b"x",
            &salt,
            &sk
        )
        .is_err());
        assert!(vapid_auth_header(
            &SigningKey::random(&mut rand::rngs::OsRng),
            "http://insecure"
        )
        .is_err());
    }
}
