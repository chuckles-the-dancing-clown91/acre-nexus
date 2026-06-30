use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One cap-table row: an owner's stake in the legal entity.
#[derive(Serialize, schemars::JsonSchema)]
pub struct CapTableRow {
    pub ownership_id: Uuid,
    pub owner_id: Uuid,
    pub owner_name: String,
    pub owner_kind: String,
    /// Ownership in basis points (10000 = 100%).
    pub ownership_bps: i32,
    /// Human label, e.g. "40.0%".
    pub ownership_label: String,
    pub role: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CapTableResp {
    pub entity_id: Uuid,
    pub rows: Vec<CapTableRow>,
    /// Sum of all stakes in basis points (a healthy cap table totals 10000).
    pub total_bps: i32,
    pub total_label: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddOwnershipReq {
    /// Reference an existing owner, or omit and provide `owner_name` to create one.
    pub owner_id: Option<Uuid>,
    pub owner_name: Option<String>,
    /// `firm` | `individual` | `company` (used when creating a new owner).
    pub owner_kind: Option<String>,
    /// Stake in basis points (10000 = 100%).
    pub ownership_bps: i32,
    /// `member` | `manager` | `investor`.
    pub role: Option<String>,
}
