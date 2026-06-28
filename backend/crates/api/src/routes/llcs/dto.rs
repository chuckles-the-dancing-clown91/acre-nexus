//! Request/response shapes for the LLC endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Full LLC profile, including the onboarding fields.
#[derive(Serialize, schemars::JsonSchema)]
pub struct LlcResp {
    pub id: Uuid,
    pub name: String,
    pub ein: String,
    pub state: String,
    pub entity_type: String,
    pub formation_date: Option<String>,
    pub registered_agent: Option<String>,
    pub principal_address: Option<String>,
    pub mailing_address: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub website: Option<String>,
    pub status: String,
    pub onboarded: bool,
}

impl From<entity::llc::Model> for LlcResp {
    fn from(l: entity::llc::Model) -> Self {
        LlcResp {
            id: l.id,
            name: l.name,
            ein: l.ein,
            state: l.state,
            entity_type: l.entity_type,
            formation_date: l.formation_date,
            registered_agent: l.registered_agent,
            principal_address: l.principal_address,
            mailing_address: l.mailing_address,
            contact_name: l.contact_name,
            contact_email: l.contact_email,
            contact_phone: l.contact_phone,
            website: l.website,
            status: l.status,
            onboarded: l.onboarded_at.is_some(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateLlcReq {
    pub name: String,
    pub ein: Option<String>,
    pub state: Option<String>,
    pub entity_type: Option<String>,
}

/// Partial update of an LLC's onboarding profile. Every field is optional; only
/// the provided ones are changed. Setting `status` to `active` stamps onboarding.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateLlcReq {
    pub name: Option<String>,
    pub ein: Option<String>,
    pub state: Option<String>,
    pub entity_type: Option<String>,
    pub formation_date: Option<String>,
    pub registered_agent: Option<String>,
    pub principal_address: Option<String>,
    pub mailing_address: Option<String>,
    pub contact_name: Option<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
    pub website: Option<String>,
    pub status: Option<String>,
}

// ---- documents ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct LlcDocumentDto {
    pub id: Uuid,
    pub llc_id: Uuid,
    pub kind: String,
    pub title: Option<String>,
    pub original_filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub storage_provider: String,
    pub verified: bool,
    pub created_at: String,
}

impl From<entity::llc_document::Model> for LlcDocumentDto {
    fn from(d: entity::llc_document::Model) -> Self {
        LlcDocumentDto {
            id: d.id,
            llc_id: d.llc_id,
            kind: d.kind,
            title: d.title,
            original_filename: d.original_filename,
            mime_type: d.mime_type,
            size_bytes: d.size_bytes,
            storage_provider: d.storage_provider,
            verified: d.verified_at.is_some(),
            created_at: d.created_at.to_rfc3339(),
        }
    }
}

// ---- branding ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct BrandingDto {
    pub llc_id: Uuid,
    pub logo_document_id: Option<Uuid>,
    pub primary_color: Option<String>,
    pub accent_color: Option<String>,
    pub signature_name: Option<String>,
    pub signature_title: Option<String>,
    pub signature_block: Option<String>,
    pub letterhead: Option<String>,
    pub footer: Option<String>,
}

impl From<entity::llc_branding::Model> for BrandingDto {
    fn from(b: entity::llc_branding::Model) -> Self {
        BrandingDto {
            llc_id: b.llc_id,
            logo_document_id: b.logo_document_id,
            primary_color: b.primary_color,
            accent_color: b.accent_color,
            signature_name: b.signature_name,
            signature_title: b.signature_title,
            signature_block: b.signature_block,
            letterhead: b.letterhead,
            footer: b.footer,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateBrandingReq {
    pub logo_document_id: Option<Uuid>,
    pub primary_color: Option<String>,
    pub accent_color: Option<String>,
    pub signature_name: Option<String>,
    pub signature_title: Option<String>,
    pub signature_block: Option<String>,
    pub letterhead: Option<String>,
    pub footer: Option<String>,
}

// ---- templates ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct TemplateDto {
    pub id: Uuid,
    pub llc_id: Uuid,
    pub kind: String,
    pub name: String,
    pub subject: Option<String>,
    pub body: String,
    pub is_default: bool,
    pub created_at: String,
}

impl From<entity::llc_template::Model> for TemplateDto {
    fn from(t: entity::llc_template::Model) -> Self {
        TemplateDto {
            id: t.id,
            llc_id: t.llc_id,
            kind: t.kind,
            name: t.name,
            subject: t.subject,
            body: t.body,
            is_default: t.is_default,
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateTemplateReq {
    pub kind: String,
    pub name: String,
    pub subject: Option<String>,
    pub body: String,
    pub is_default: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateTemplateReq {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub is_default: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PreviewReq {
    /// Raw Handlebars body to render (lets the UI preview unsaved edits).
    pub body: String,
    /// Extra merge context to overlay on the LLC/branding base context.
    pub context: Option<serde_json::Value>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct PreviewResp {
    pub rendered: String,
}

// ---- generation ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct GenerateReq {
    /// Template to render. If omitted, a sensible built-in default for `kind`.
    pub template_id: Option<Uuid>,
    /// `lease` | `letter` (default `letter`).
    pub kind: Option<String>,
    pub title: Option<String>,
    /// When set, pull lease/property facts into the context (for lease contracts).
    pub lease_id: Option<Uuid>,
    pub recipient_name: Option<String>,
    pub recipient_email: Option<String>,
    pub property_address: Option<String>,
    /// Arbitrary extra merge fields.
    pub context: Option<serde_json::Value>,
    /// Also dispatch the rendered document to `recipient_email` via the email job.
    pub send_email: Option<bool>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct GeneratedDocumentDto {
    pub id: Uuid,
    pub llc_id: Uuid,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub lease_id: Option<Uuid>,
    pub created_at: String,
}

impl From<entity::generated_document::Model> for GeneratedDocumentDto {
    fn from(g: entity::generated_document::Model) -> Self {
        GeneratedDocumentDto {
            id: g.id,
            llc_id: g.llc_id,
            kind: g.kind,
            title: g.title,
            status: g.status,
            mime_type: g.mime_type,
            size_bytes: g.size_bytes,
            lease_id: g.lease_id,
            created_at: g.created_at.to_rfc3339(),
        }
    }
}

// ---- per-tenant storage configuration ----

#[derive(Serialize, schemars::JsonSchema)]
pub struct StorageConfigDto {
    /// `platform` | `local` | `s3` | `gcs`.
    pub provider: String,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub prefix: Option<String>,
    pub endpoint: Option<String>,
    /// Whether bring-your-own credentials are set (the secret itself is never returned).
    pub has_credentials: bool,
    /// True when falling back to the platform-managed default (no override row).
    pub is_default: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateStorageConfigReq {
    pub provider: String,
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub prefix: Option<String>,
    pub endpoint: Option<String>,
    /// Raw credential blob: for `s3`, `{"access_key_id":"…","secret_access_key":"…"}`;
    /// for `gcs`, the service-account JSON. Sealed (AES-256-GCM) before storage.
    pub secret: Option<String>,
}
