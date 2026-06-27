//! Request/response shapes for the LLC endpoints.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, schemars::JsonSchema)]
pub struct LlcResp {
    pub id: Uuid,
    pub name: String,
    pub ein: String,
    pub state: String,
}

impl From<entity::llc::Model> for LlcResp {
    fn from(l: entity::llc::Model) -> Self {
        LlcResp {
            id: l.id,
            name: l.name,
            ein: l.ein,
            state: l.state,
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateLlcReq {
    pub name: String,
    pub ein: Option<String>,
    pub state: Option<String>,
}
