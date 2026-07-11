use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---- commitments ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddCommitmentReq {
    /// Reference an existing owner, or omit and provide `owner_name` to create one.
    pub owner_id: Option<Uuid>,
    pub owner_name: Option<String>,
    /// `firm` | `individual` | `company` (used when creating a new owner).
    pub owner_kind: Option<String>,
    /// `investor` (LP) | `manager` (GP) | `member`.
    pub role: Option<String>,
    pub committed_cents: i64,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CommitmentDto {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub owner_name: String,
    pub role: String,
    pub committed_cents: i64,
    pub contributed_cents: i64,
    pub returned_cents: i64,
    /// Contributed capital not yet returned (`contributed - returned`).
    pub unreturned_cents: i64,
    pub status: String,
}

impl CommitmentDto {
    pub fn build(m: &entity::investor_commitment::Model, owner_name: String) -> Self {
        CommitmentDto {
            id: m.id,
            owner_id: m.owner_id,
            owner_name,
            role: m.role.clone(),
            committed_cents: m.committed_cents,
            contributed_cents: m.contributed_cents,
            returned_cents: m.returned_cents,
            unreturned_cents: (m.contributed_cents - m.returned_cents).max(0),
            status: m.status.clone(),
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CommitmentListResp {
    pub entity_id: Uuid,
    pub commitments: Vec<CommitmentDto>,
    pub total_committed_cents: i64,
    pub total_contributed_cents: i64,
}

// ---- capital calls ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateCapitalCallReq {
    pub amount_cents: i64,
    pub due_date: Option<String>,
    pub memo: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CapitalCallLineDto {
    pub id: Uuid,
    pub commitment_id: Uuid,
    pub owner_id: Uuid,
    pub owner_name: String,
    pub amount_cents: i64,
    pub status: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct CapitalCallDto {
    pub id: Uuid,
    pub number: i32,
    pub amount_cents: i64,
    pub status: String,
    pub due_date: Option<String>,
    pub memo: Option<String>,
    pub lines: Vec<CapitalCallLineDto>,
}

// ---- distributions ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateDistributionReq {
    pub amount_cents: i64,
    /// Preferred-return rate in basis points (default 0).
    pub pref_rate_bps: Option<i32>,
    /// GP carried interest in basis points (default 0).
    pub carry_bps: Option<i32>,
    pub memo: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct DistributionLineDto {
    pub id: Uuid,
    pub commitment_id: Uuid,
    pub owner_id: Uuid,
    pub owner_name: String,
    pub return_of_capital_cents: i64,
    pub preferred_cents: i64,
    pub profit_cents: i64,
    pub carry_cents: i64,
    pub total_cents: i64,
}

impl DistributionLineDto {
    pub fn build(m: &entity::distribution_line::Model, owner_name: String) -> Self {
        DistributionLineDto {
            id: m.id,
            commitment_id: m.commitment_id,
            owner_id: m.owner_id,
            owner_name,
            return_of_capital_cents: m.return_of_capital_cents,
            preferred_cents: m.preferred_cents,
            profit_cents: m.profit_cents,
            carry_cents: m.carry_cents,
            total_cents: m.total_cents,
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct DistributionDto {
    pub id: Uuid,
    pub number: i32,
    pub amount_cents: i64,
    pub pref_rate_bps: i32,
    pub carry_bps: i32,
    pub memo: Option<String>,
    pub lines: Vec<DistributionLineDto>,
}
