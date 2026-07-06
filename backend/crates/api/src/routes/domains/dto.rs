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
    /// Whether SPF + DKIM + DMARC have all verified for branded sending.
    pub email_verified: bool,
    pub email_verified_at: Option<String>,
    /// Per-record email DNS check results (`spf` / `dkim` / `dmarc`).
    pub email_dns_status: serde_json::Value,
    /// The SPF/DKIM/DMARC records the firm must publish for branded mail from
    /// a custom domain to pass authentication.
    pub email_dns_records: Option<Vec<EmailDnsRecord>>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct DnsInstructions {
    pub cname_target: String,
    pub txt_name: String,
    pub txt_value: String,
}

/// One TXT record to publish for email deliverability.
#[derive(Clone, Serialize, schemars::JsonSchema)]
pub struct EmailDnsRecord {
    /// `spf` | `dkim` | `dmarc`.
    pub key: String,
    /// The DNS name to create the TXT record at.
    pub name: String,
    /// The TXT value to publish.
    pub value: String,
    /// The substring the verification check looks for.
    #[serde(skip)]
    pub expect_contains: String,
}

/// The SPF/DKIM/DMARC set for a custom sending domain. The DKIM value is
/// derived deterministically per (tenant, hostname) — in production the
/// selector's key material is what the configured ESP publishes for you.
pub fn email_dns_records(tenant_id: Uuid, hostname: &str) -> Vec<EmailDnsRecord> {
    let dkim_token =
        crate::storage::sha256_hex(format!("acre-dkim:{tenant_id}:{hostname}").as_bytes());
    let dkim_value = format!("v=DKIM1; k=rsa; p={}", &dkim_token[..32]);
    vec![
        EmailDnsRecord {
            key: "spf".into(),
            name: hostname.to_string(),
            value: "v=spf1 include:spf.acrenexus.com ~all".into(),
            expect_contains: "include:spf.acrenexus.com".into(),
        },
        EmailDnsRecord {
            key: "dkim".into(),
            name: format!("acre._domainkey.{hostname}"),
            expect_contains: dkim_value.clone(),
            value: dkim_value,
        },
        EmailDnsRecord {
            key: "dmarc".into(),
            name: format!("_dmarc.{hostname}"),
            value: "v=DMARC1; p=quarantine; rua=mailto:dmarc@acrenexus.com".into(),
            expect_contains: "v=DMARC1".into(),
        },
    ]
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
        // Branded sending is a custom-domain concern; platform subdomains
        // already authenticate under the platform's own records.
        let email_dns = (d.kind == "custom").then(|| email_dns_records(d.tenant_id, &d.hostname));
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
            email_verified: d.email_verified_at.is_some(),
            email_verified_at: d.email_verified_at.map(|x| x.to_rfc3339()),
            email_dns_status: d.email_dns_status,
            email_dns_records: email_dns,
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
