//! Request/response shapes for the LLC endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct LlcResp {
    pub id: Uuid,
    pub name: String,
    pub ein: String,
    pub state: String,
    pub entity_type: String,
    pub registered_agent: Option<String>,
    pub status: String,
}

impl From<entity::llc::Model> for LlcResp {
    fn from(l: entity::llc::Model) -> Self {
        LlcResp {
            id: l.id,
            name: l.name,
            ein: l.ein,
            state: l.state,
            entity_type: l.entity_type,
            registered_agent: l.registered_agent,
            status: l.status,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateLlcReq {
    pub name: String,
    pub ein: Option<String>,
    pub state: Option<String>,
    /// `llc` | `lp` | `s_corp` | `c_corp` | `sole_prop` (defaults to `llc`).
    pub entity_type: Option<String>,
    pub registered_agent: Option<String>,
}
