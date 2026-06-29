//! Request/response shapes for white-label domain management.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct DomainResp {
    pub id: Uuid,
    pub hostname: String,
    pub kind: String,
    pub audience: String,
    pub verification_token: Option<String>,
    pub verified: bool,
    pub verified_at: Option<String>,
    pub tls_status: String,
    /// DNS record the firm must publish to verify a custom domain (CNAME + TXT).
    pub dns_instructions: Option<DnsInstructions>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct DnsInstructions {
    pub cname_target: String,
    pub txt_name: String,
    pub txt_value: String,
}

impl From<entity::domain::Model> for DomainResp {
    fn from(d: entity::domain::Model) -> Self {
        let dns = match (d.kind.as_str(), &d.verification_token) {
            ("custom", Some(token)) => Some(DnsInstructions {
                cname_target: "edge.acrenexus.com".into(),
                txt_name: format!("_acre-challenge.{}", d.hostname),
                txt_value: token.clone(),
            }),
            _ => None,
        };
        DomainResp {
            id: d.id,
            hostname: d.hostname,
            kind: d.kind,
            audience: d.audience,
            verification_token: d.verification_token,
            verified: d.verified_at.is_some(),
            verified_at: d.verified_at.map(|x| x.to_rfc3339()),
            tls_status: d.tls_status,
            dns_instructions: dns,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateDomainReq {
    pub hostname: String,
    /// `admin` | `owner` | `renter` (defaults to `admin`).
    pub audience: Option<String>,
}

/// What a host resolves to — for the unauthenticated routing layer (§7.2).
#[derive(Serialize, schemars::JsonSchema)]
pub struct ResolveResp {
    pub tenant_id: Uuid,
    pub tenant_slug: String,
    pub audience: String,
    pub company_name: String,
    pub primary_color: String,
    pub accent_color: String,
}
