//! Request/response shapes for CRM leads.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Lead statuses, in pipeline order.
pub const STATUSES: &[&str] = &["new", "contacted", "toured", "applied", "closed"];

#[derive(Serialize, schemars::JsonSchema)]
pub struct LeadDto {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
    pub source: String,
    pub status: String,
    pub notes: Option<String>,
    pub last_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::lead::Model> for LeadDto {
    fn from(l: entity::lead::Model) -> Self {
        LeadDto {
            id: l.id,
            name: l.name,
            email: l.email,
            phone: l.phone,
            source: l.source,
            status: l.status,
            notes: l.notes,
            last_message: l.last_message,
            created_at: l.created_at.to_rfc3339(),
            updated_at: l.updated_at.to_rfc3339(),
        }
    }
}

/// The leads list plus the tenant's monitored inbox address (mail sent there
/// creates/updates leads).
#[derive(Serialize, schemars::JsonSchema)]
pub struct LeadsResp {
    pub inbox_address: Option<String>,
    pub leads: Vec<LeadDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateLeadReq {
    pub name: Option<String>,
    pub phone: Option<String>,
    /// `new` | `contacted` | `toured` | `applied` | `closed`.
    pub status: Option<String>,
    pub notes: Option<String>,
}
