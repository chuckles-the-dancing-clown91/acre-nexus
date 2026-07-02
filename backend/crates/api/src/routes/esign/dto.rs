//! Request/response shapes for the e-signature envelope endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Console (authenticated) shapes
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct SignerDto {
    pub id: Uuid,
    pub role: String,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    /// `sent` | `viewed` | `signed` | `declined`.
    pub status: String,
    pub viewed_at: Option<String>,
    pub signed_at: Option<String>,
    pub signed_name: Option<String>,
    pub decline_reason: Option<String>,
}

impl From<entity::esign_signer::Model> for SignerDto {
    fn from(s: entity::esign_signer::Model) -> Self {
        SignerDto {
            id: s.id,
            role: s.role,
            name: s.name,
            email: s.email,
            phone: s.phone,
            status: s.status,
            viewed_at: s.viewed_at.map(|x| x.to_rfc3339()),
            signed_at: s.signed_at.map(|x| x.to_rfc3339()),
            signed_name: s.signed_name,
            decline_reason: s.decline_reason,
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct EsignEventDto {
    pub id: Uuid,
    pub signer_id: Option<Uuid>,
    pub event: String,
    pub detail: serde_json::Value,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: String,
}

impl From<entity::esign_event::Model> for EsignEventDto {
    fn from(e: entity::esign_event::Model) -> Self {
        EsignEventDto {
            id: e.id,
            signer_id: e.signer_id,
            event: e.event,
            detail: e.detail,
            ip: e.ip,
            user_agent: e.user_agent,
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct EnvelopeDto {
    pub id: Uuid,
    pub lease_id: Uuid,
    pub lease_document_id: Uuid,
    pub title: String,
    pub message: Option<String>,
    /// `sent` | `partially_signed` | `completed` | `declined` | `voided`.
    pub status: String,
    /// SHA-256 of the document body every signer signs.
    pub body_hash: String,
    /// The stored signed PDF (document service id) once completed.
    pub signed_document_id: Option<Uuid>,
    pub sent_at: String,
    pub completed_at: Option<String>,
    pub voided_at: Option<String>,
    pub void_reason: Option<String>,
    pub signers: Vec<SignerDto>,
    /// The ESIGN/UETA audit trail, newest first.
    pub events: Vec<EsignEventDto>,
}

impl EnvelopeDto {
    pub fn build(
        envelope: entity::esign_envelope::Model,
        signers: Vec<entity::esign_signer::Model>,
        events: Vec<entity::esign_event::Model>,
    ) -> Self {
        EnvelopeDto {
            id: envelope.id,
            lease_id: envelope.lease_id,
            lease_document_id: envelope.lease_document_id,
            title: envelope.title,
            message: envelope.message,
            status: envelope.status,
            body_hash: envelope.body_hash,
            signed_document_id: envelope.signed_document_id,
            sent_at: envelope.sent_at.to_rfc3339(),
            completed_at: envelope.completed_at.map(|x| x.to_rfc3339()),
            voided_at: envelope.voided_at.map(|x| x.to_rfc3339()),
            void_reason: envelope.void_reason,
            signers: signers.into_iter().map(SignerDto::from).collect(),
            events: events.into_iter().map(EsignEventDto::from).collect(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SignerReq {
    /// `resident` | `landlord` | `guarantor` | `other`.
    pub role: Option<String>,
    pub name: String,
    pub email: String,
    /// Optional mobile — when present the signing link also goes out by SMS.
    pub phone: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateEnvelopeReq {
    /// Optional note shown to signers.
    pub message: Option<String>,
    /// Explicit signer list. Omitted → the lease's resident plus the sending
    /// user (as landlord).
    pub signers: Option<Vec<SignerReq>>,
}

/// One signer's freshly-minted signing link. **Returned exactly once** (at
/// send/remind time) — only the token's hash is stored.
#[derive(Serialize, schemars::JsonSchema)]
pub struct SignerLink {
    pub signer_id: Uuid,
    pub name: String,
    pub email: String,
    pub sign_url: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CreateEnvelopeResp {
    pub envelope: EnvelopeDto,
    /// Signing links, for copy/paste — also emailed (and texted) to signers.
    pub sign_links: Vec<SignerLink>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct VoidReq {
    pub reason: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct RemindResp {
    pub reminded: usize,
    /// Fresh signing links (reminders rotate each pending signer's token).
    pub sign_links: Vec<SignerLink>,
}

// ---------------------------------------------------------------------------
// Public (tokenized signer) shapes
// ---------------------------------------------------------------------------

/// A co-signer as shown to another signer: status only, no contact details.
#[derive(Serialize, schemars::JsonSchema)]
pub struct PublicCoSigner {
    pub name: String,
    pub role: String,
    pub status: String,
}

/// Everything the public signing page needs, scoped to one signer's token.
#[derive(Serialize, schemars::JsonSchema)]
pub struct PublicSignView {
    /// Workspace branding for the signing page header.
    pub company: String,
    pub envelope_status: String,
    pub document_title: String,
    /// The full document text (exactly what is being signed). Omitted when the
    /// envelope was voided.
    pub document_body: Option<String>,
    /// SHA-256 of the document body — shown for integrity transparency.
    pub body_hash: String,
    pub message: Option<String>,
    /// This signer (the token holder).
    pub signer: SignerDto,
    pub co_signers: Vec<PublicCoSigner>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SubmitSignatureReq {
    /// The typed full-name signature.
    pub signed_name: String,
    /// Explicit ESIGN/UETA consent to transact electronically.
    pub consent: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DeclineReq {
    pub reason: Option<String>,
}
