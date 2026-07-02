//! **E-signature envelope engine** (roadmap Phase 2 — contract signing).
//!
//! A generated lease document is sent out as an *envelope* to one or more
//! *signers* (resident, landlord, guarantor, …). Each signer receives a
//! tokenized signing link by email (and SMS when a phone is on file) through
//! the Phase 1 notification substrate. The token is stored hashed (SHA-256,
//! for lookup) plus sealed under the integration-secrets key (AES-256-GCM,
//! for re-delivery) — never plaintext at rest — so reminders re-send the
//! **same** link rather than invalidating earlier emails.
//!
//! State machine:
//!
//! ```text
//! envelope: sent ──→ partially_signed ──→ completed
//!             │                            (all signers signed: lease doc
//!             ├──→ declined                 marked signed, lease activated,
//!             └──→ voided                   signed PDF stored, staff+signers
//! signer:   sent ──→ viewed ──→ signed      notified, workflow advanced)
//!             └────────────└──→ declined
//! ```
//!
//! Every transition appends an [`entity::esign_event`] row (IP + user agent)
//! — the ESIGN/UETA audit trail — and the envelope pins a SHA-256 of the
//! document body at send time so all parties provably signed the same text.

use crate::pdf;
use crate::storage::{sha256_hex, ObjectStore};
use chrono::Utc;
use entity::prelude::{Document, EsignSigner, Lease, LeaseDocument, Property, Tenant};
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde_json::json;
use uuid::Uuid;

/// Signer roles the envelope flow understands.
pub const SIGNER_ROLES: &[&str] = &["resident", "landlord", "guarantor", "other"];

/// The filename the completed, signed rendition is stored under (per lease,
/// versioned by the document service on re-signing).
pub const SIGNED_PDF_FILENAME: &str = "signed-lease-agreement.pdf";

// ---------------------------------------------------------------------------
// Tokens + links
// ---------------------------------------------------------------------------

/// Mint a signing-link token: 32 random bytes, hex — returned raw (for the
/// link) plus its SHA-256 (the lookup form we persist).
pub fn generate_token() -> (String, String) {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    let raw: String = buf.iter().map(|b| format!("{b:02x}")).collect();
    let hash = hash_token(&raw);
    (raw, hash)
}

/// SHA-256 (hex) of a raw signing token.
pub fn hash_token(raw: &str) -> String {
    sha256_hex(raw.as_bytes())
}

/// Seal a raw token with AES-256-GCM under the integration-secrets key
/// (never plaintext at rest) → `(ciphertext_b64, nonce_b64)`. Reminders
/// unseal it to re-send the **same** link instead of rotating it.
pub fn seal_token(raw: &str) -> anyhow::Result<(String, String)> {
    let sealed = crate::pii::encrypt(&crate::config::Config::global().secrets_key, raw)?;
    Ok((sealed.ciphertext, sealed.nonce))
}

/// Recover a signer's raw token from its seal.
pub fn unseal_token(ciphertext_b64: &str, nonce_b64: &str) -> anyhow::Result<String> {
    crate::pii::decrypt(
        &crate::config::Config::global().secrets_key,
        ciphertext_b64,
        nonce_b64,
    )
}

/// The public signing URL for a token: `{PUBLIC_APP_URL}/sign/{token}?tenant={slug}`.
/// The tenant slug rides along so the unauthenticated page can resolve the
/// workspace (same contract as the public apply funnel).
pub fn sign_url(tenant_slug: &str, raw_token: &str) -> String {
    let base = std::env::var("PUBLIC_APP_URL")
        .unwrap_or_else(|_| "http://localhost:3000".into())
        .trim_end_matches('/')
        .to_string();
    format!("{base}/sign/{raw_token}?tenant={tenant_slug}")
}

/// The tenant's slug (for building signing links). Falls back to the id.
pub async fn tenant_slug(db: &impl ConnectionTrait, tenant_id: Uuid) -> String {
    Tenant::find_by_id(tenant_id)
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|t| t.slug)
        .unwrap_or_else(|| tenant_id.to_string())
}

// ---------------------------------------------------------------------------
// Audit trail
// ---------------------------------------------------------------------------

/// Append one event to the envelope's ESIGN/UETA audit trail (best-effort:
/// a failed insert is logged, never fails the transition it documents).
#[allow(clippy::too_many_arguments)]
pub async fn record_event(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    envelope_id: Uuid,
    signer_id: Option<Uuid>,
    event: &str,
    detail: serde_json::Value,
    ip: Option<String>,
    user_agent: Option<String>,
) {
    let row = entity::esign_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        envelope_id: Set(envelope_id),
        signer_id: Set(signer_id),
        event: Set(event.to_string()),
        detail: Set(detail),
        ip: Set(ip),
        user_agent: Set(user_agent),
        created_at: Set(Utc::now().into()),
    };
    if let Err(e) = row.insert(db).await {
        tracing::error!("failed to record esign event '{event}': {e}");
    }
}

// ---------------------------------------------------------------------------
// Signer notifications (email + SMS via the Phase 1 queue)
// ---------------------------------------------------------------------------

/// Queue the signing-link email (and SMS when a phone is on file) for one
/// signer. `trigger` feeds the notification engine's idempotency key, so the
/// initial send and each reminder are distinct sends but retries never
/// double-deliver.
pub async fn notify_signer(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    signer: &entity::esign_signer::Model,
    template: &str,
    trigger: &str,
    vars: serde_json::Value,
) {
    let base = json!({
        "template": template,
        "owner_type": "esign_signer",
        "owner_id": signer.id,
        "trigger": trigger,
        "vars": vars,
    });

    let mut email = base.clone();
    email["to"] = json!(signer.email);
    if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_email", email, 0).await {
        tracing::error!("failed to enqueue esign email for {}: {e}", signer.id);
    }

    if let Some(phone) = signer.phone.as_deref().filter(|p| !p.trim().is_empty()) {
        let mut sms = base;
        sms["to"] = json!(phone);
        if let Err(e) = crate::scheduler::enqueue(db, tenant_id, "auto_sms", sms, 0).await {
            tracing::error!("failed to enqueue esign sms for {}: {e}", signer.id);
        }
    }
}

// ---------------------------------------------------------------------------
// Completion
// ---------------------------------------------------------------------------

/// The plain-text **signature certificate** appended to the signed rendition:
/// who signed, when, from where — the human-readable form of the audit trail.
pub fn signature_certificate(
    envelope: &entity::esign_envelope::Model,
    signers: &[entity::esign_signer::Model],
) -> String {
    let mut cert = String::new();
    cert.push_str("\n\n==========================================================\n");
    cert.push_str("SIGNATURE CERTIFICATE (ESIGN / UETA)\n");
    cert.push_str("==========================================================\n");
    cert.push_str(&format!("Envelope: {}\n", envelope.id));
    cert.push_str(&format!(
        "Document integrity: SHA-256 {}\n",
        envelope.body_hash
    ));
    cert.push_str(&format!("Sent: {}\n", envelope.sent_at.to_rfc3339()));
    for s in signers {
        cert.push_str("----------------------------------------------------------\n");
        cert.push_str(&format!("Signer: {} <{}> ({})\n", s.name, s.email, s.role));
        cert.push_str(&format!(
            "Signature (typed): {}\n",
            s.signed_name.as_deref().unwrap_or("—")
        ));
        if let Some(at) = &s.signed_at {
            cert.push_str(&format!("Signed at: {}\n", at.to_rfc3339()));
        }
        if let Some(ip) = &s.signed_ip {
            cert.push_str(&format!("IP address: {ip}\n"));
        }
        if let Some(ua) = &s.signed_user_agent {
            cert.push_str(&format!("User agent: {ua}\n"));
        }
    }
    cert.push_str("----------------------------------------------------------\n");
    cert.push_str(
        "All parties consented to transact electronically. This record and its \
         audit trail are retained by the platform.\n",
    );
    cert
}

/// True when every signer on the envelope has signed.
pub fn all_signed(signers: &[entity::esign_signer::Model]) -> bool {
    !signers.is_empty() && signers.iter().all(|s| s.status == "signed")
}

/// Finish a fully-signed envelope: mark the lease document signed, activate
/// the lease (+ occupancy sync), store the signed PDF in the document
/// service, advance the property's workflow toward `leased`, append the
/// audit-trail event, and notify signers + staff. Returns the stored signed
/// document's id.
pub async fn complete_envelope(
    db: &impl ConnectionTrait,
    envelope: &entity::esign_envelope::Model,
    signers: &[entity::esign_signer::Model],
) -> anyhow::Result<Uuid> {
    let now = Utc::now();
    let tenant_id = envelope.tenant_id;

    let doc = LeaseDocument::find_by_id(envelope.lease_document_id)
        .filter(entity::lease_document::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("lease document vanished"))?;
    let lease = Lease::find_by_id(envelope.lease_id)
        .filter(entity::lease::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("lease vanished"))?;

    // 1. The lease document is now signed — by every party on the envelope.
    let signed_by = signers
        .iter()
        .map(|s| s.signed_name.clone().unwrap_or_else(|| s.name.clone()))
        .collect::<Vec<_>>()
        .join("; ");
    let last_ip = signers.iter().rev().find_map(|s| s.signed_ip.clone());
    let body = doc.body.clone();
    let title = doc.title.clone();
    let mut dm: entity::lease_document::ActiveModel = doc.into();
    dm.status = Set("signed".into());
    dm.signed_at = Set(Some(now.into()));
    dm.signed_by = Set(Some(signed_by.clone()));
    dm.signed_hash = Set(Some(envelope.body_hash.clone()));
    dm.signed_ip = Set(last_ip);
    dm.update(db).await?;

    // 2. Signing activates the tenancy (same rule as in-person signing); the
    //    advertised listing (if the lease came from one) closes out.
    let property_id = lease.property_id;
    let lease = if lease.status != "active" {
        let mut lm: entity::lease::ActiveModel = lease.into();
        lm.status = Set("active".into());
        lm.updated_at = Set(now.into());
        lm.update(db).await?
    } else {
        lease
    };
    crate::rentals_occupancy::sync_property_occupancy(db, property_id).await;
    crate::listing_sync::close_on_lease_activation(db, tenant_id, &lease).await;

    // 3. Store the signed rendition (body + signature certificate) as a PDF in
    //    the document service, versioned like any other upload.
    let full_text = format!("{body}{}", signature_certificate(envelope, signers));
    let pdf_bytes = pdf::text_to_pdf(&full_text);
    let signed_doc_id = store_signed_pdf(db, tenant_id, envelope.lease_id, &pdf_bytes).await?;

    // 4. The envelope itself is done.
    let mut em: entity::esign_envelope::ActiveModel = envelope.clone().into();
    em.status = Set("completed".into());
    em.completed_at = Set(Some(now.into()));
    em.signed_document_id = Set(Some(signed_doc_id));
    em.updated_at = Set(now.into());
    em.update(db).await?;

    // 5. The property's process advances toward "leased" automatically, so the
    //    tracker reflects what actually happened.
    advance_workflow_on_lease_signed(db, tenant_id, property_id, &signed_by).await;

    // 6. Audit trail + platform audit log.
    record_event(
        db,
        tenant_id,
        envelope.id,
        None,
        "completed",
        json!({ "signed_by": signed_by, "signed_document_id": signed_doc_id }),
        None,
        None,
    )
    .await;
    crate::audit::record(
        db,
        None,
        crate::audit::actions::ESIGN_COMPLETE,
        Some("esign_envelope"),
        Some(envelope.id.to_string()),
        Some(tenant_id),
        Some(json!({
            "lease_id": envelope.lease_id,
            "signed_document_id": signed_doc_id,
            "signers": signers.len(),
        })),
    )
    .await;

    // 7. Everyone hears about it: each signer by email/SMS, staff via the
    //    integrated in-app/push/chat fan-out.
    for s in signers {
        notify_signer(
            db,
            tenant_id,
            s,
            "esign_completed",
            "completed",
            json!({ "document_title": title, "signer": s.name }),
        )
        .await;
    }
    crate::notify::notify_staff(
        db,
        tenant_id,
        "lease:read",
        "esign_completed_staff",
        json!({ "document_title": title, "signed_by": signed_by }),
        Some(("esign_envelope", envelope.id)),
        "completed",
        None,
    )
    .await;

    Ok(signed_doc_id)
}

/// Insert the signed PDF into the document service (new version if one
/// already exists for this lease) and write the bytes server-side.
async fn store_signed_pdf(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    lease_id: Uuid,
    bytes: &[u8],
) -> anyhow::Result<Uuid> {
    let previous = Document::find()
        .filter(entity::document::Column::TenantId.eq(tenant_id))
        .filter(entity::document::Column::OwnerType.eq("lease"))
        .filter(entity::document::Column::OwnerId.eq(lease_id))
        .filter(entity::document::Column::Filename.eq(SIGNED_PDF_FILENAME))
        .order_by_desc(entity::document::Column::Version)
        .one(db)
        .await?;
    let (version, previous_version_id) = match &previous {
        Some(p) => (p.version + 1, Some(p.id)),
        None => (1, None),
    };

    let id = Uuid::new_v4();
    let storage_key = format!("{tenant_id}/{id}");
    let store = ObjectStore::from_env()?;
    store.put_bytes(&storage_key, bytes).await?;

    let now = Utc::now();
    entity::document::ActiveModel {
        id: Set(id),
        tenant_id: Set(tenant_id),
        owner_type: Set("lease".into()),
        owner_id: Set(lease_id),
        filename: Set(SIGNED_PDF_FILENAME.into()),
        mime_type: Set("application/pdf".into()),
        size_bytes: Set(bytes.len() as i64),
        checksum: Set(Some(sha256_hex(bytes))),
        version: Set(version),
        previous_version_id: Set(previous_version_id),
        storage_key: Set(storage_key),
        status: Set("stored".into()),
        retention_expires_at: Set(None),
        created_by: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await?;
    Ok(id)
}

/// When a lease is signed, move the property's investment workflow forward to
/// its `leased` stage (if the strategy has one and the property hasn't reached
/// it yet), recording the transition like a manual advance would.
async fn advance_workflow_on_lease_signed(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    property_id: Uuid,
    signed_by: &str,
) {
    let Ok(Some(property)) = Property::find_by_id(property_id)
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
    else {
        return;
    };
    let Some(strategy) = crate::workflow::strategy(&property.strategy) else {
        return;
    };
    let stage_keys: Vec<&str> = strategy.stages.iter().map(|s| s.key).collect();
    let Some(leased_idx) = stage_keys.iter().position(|k| *k == "leased") else {
        return;
    };
    let current_idx = stage_keys
        .iter()
        .position(|k| *k == property.workflow_stage);
    // Only ever move forward; a property already at/past "leased" stays put.
    if matches!(current_idx, Some(ci) if ci >= leased_idx) {
        return;
    }

    let from_stage = (!property.workflow_stage.is_empty()).then(|| property.workflow_stage.clone());
    let strategy_key = property.strategy.clone();
    let mut am: entity::property::ActiveModel = property.into();
    am.workflow_stage = Set("leased".into());
    if am.update(db).await.is_err() {
        return;
    }
    let event = entity::workflow_event::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        property_id: Set(property_id),
        strategy: Set(strategy_key),
        from_stage: Set(from_stage),
        to_stage: Set("leased".into()),
        note: Set(Some(format!("Lease e-signed by {signed_by}"))),
        actor_user_id: Set(None),
        created_at: Set(Utc::now().into()),
    };
    if let Err(e) = event.insert(db).await {
        tracing::warn!("failed to record auto workflow event: {e}");
    }
}

/// Load an envelope's signers, oldest first (stable display order).
pub async fn envelope_signers(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    envelope_id: Uuid,
) -> Result<Vec<entity::esign_signer::Model>, sea_orm::DbErr> {
    EsignSigner::find()
        .filter(entity::esign_signer::Column::TenantId.eq(tenant_id))
        .filter(entity::esign_signer::Column::EnvelopeId.eq(envelope_id))
        .order_by_asc(entity::esign_signer::Column::CreatedAt)
        .all(db)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_random_and_hash_deterministically() {
        let (raw1, hash1) = generate_token();
        let (raw2, hash2) = generate_token();
        assert_eq!(raw1.len(), 64);
        assert_ne!(raw1, raw2);
        assert_ne!(hash1, hash2);
        assert_eq!(hash_token(&raw1), hash1);
        assert_ne!(
            hash_token(&raw1),
            raw1,
            "the stored form is never the raw token"
        );
    }

    #[test]
    fn sealed_tokens_roundtrip_and_are_not_plaintext() {
        let (raw, _) = generate_token();
        let (ct, nonce) = seal_token(&raw).unwrap();
        assert_ne!(ct, raw, "ciphertext must not be the raw token");
        assert_eq!(unseal_token(&ct, &nonce).unwrap(), raw);
        // A seal under a different nonce must not decrypt.
        let (_, other_nonce) = seal_token(&raw).unwrap();
        assert!(unseal_token(&ct, &other_nonce).is_err());
    }

    #[test]
    fn sign_url_carries_token_and_tenant() {
        let url = sign_url("northwind", "abc123");
        assert!(url.ends_with("/sign/abc123?tenant=northwind"));
    }

    fn signer(status: &str) -> entity::esign_signer::Model {
        let now = Utc::now();
        entity::esign_signer::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            envelope_id: Uuid::new_v4(),
            role: "resident".into(),
            name: "Jordan Renter".into(),
            email: "jordan@example.com".into(),
            phone: None,
            token_hash: "x".into(),
            token_ciphertext: "ct".into(),
            token_nonce: "n".into(),
            status: status.into(),
            viewed_at: None,
            signed_at: Some(now.into()),
            signed_name: Some("Jordan Renter".into()),
            signed_ip: Some("203.0.113.7".into()),
            signed_user_agent: Some("UnitTest/1.0".into()),
            decline_reason: None,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    #[test]
    fn all_signed_requires_every_signer() {
        assert!(!all_signed(&[]));
        assert!(all_signed(&[signer("signed"), signer("signed")]));
        assert!(!all_signed(&[signer("signed"), signer("viewed")]));
    }

    #[test]
    fn certificate_carries_the_audit_essentials() {
        let now = Utc::now();
        let env = entity::esign_envelope::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            lease_id: Uuid::new_v4(),
            lease_document_id: Uuid::new_v4(),
            title: "Residential Lease Agreement".into(),
            message: None,
            status: "completed".into(),
            body_hash: "deadbeef".into(),
            signed_document_id: None,
            created_by: None,
            sent_at: now.into(),
            completed_at: Some(now.into()),
            voided_at: None,
            void_reason: None,
            created_at: now.into(),
            updated_at: now.into(),
        };
        let cert = signature_certificate(&env, &[signer("signed")]);
        assert!(cert.contains("SIGNATURE CERTIFICATE"));
        assert!(cert.contains("SHA-256 deadbeef"));
        assert!(cert.contains("Jordan Renter <jordan@example.com> (resident)"));
        assert!(cert.contains("203.0.113.7"));
        assert!(cert.contains("UnitTest/1.0"));
    }
}
