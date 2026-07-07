//! Request/response shapes for inspections + deposit disposition.

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Inspections
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct InspectionItemDto {
    pub id: Uuid,
    pub inspection_id: Uuid,
    pub area: String,
    pub item: String,
    /// `unrated` | `good` | `fair` | `poor` | `damaged`.
    pub condition: String,
    pub notes: Option<String>,
    pub sort_order: i32,
}

impl From<entity::inspection_item::Model> for InspectionItemDto {
    fn from(i: entity::inspection_item::Model) -> Self {
        InspectionItemDto {
            id: i.id,
            inspection_id: i.inspection_id,
            area: i.area,
            item: i.item,
            condition: i.condition,
            notes: i.notes,
            sort_order: i.sort_order,
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct InspectionDto {
    pub id: Uuid,
    pub lease_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    /// `move_in` | `move_out`.
    pub kind: String,
    /// `draft` | `completed`.
    pub status: String,
    pub scheduled_date: Option<String>,
    pub completed_at: Option<String>,
    pub notes: Option<String>,
    pub item_count: i64,
    /// Items rated something other than `unrated`.
    pub rated_count: i64,
    pub created_at: String,
}

pub fn inspection_dto(
    i: entity::inspection::Model,
    item_count: i64,
    rated_count: i64,
) -> InspectionDto {
    InspectionDto {
        id: i.id,
        lease_id: i.lease_id,
        property_id: i.property_id,
        unit_id: i.unit_id,
        kind: i.kind,
        status: i.status,
        scheduled_date: i.scheduled_date,
        completed_at: i.completed_at.map(|t| t.to_rfc3339()),
        notes: i.notes,
        item_count,
        rated_count,
        created_at: i.created_at.to_rfc3339(),
    }
}

/// An inspection plus its full checklist.
#[derive(Serialize, schemars::JsonSchema)]
pub struct InspectionDetailDto {
    #[serde(flatten)]
    pub inspection: InspectionDto,
    pub items: Vec<InspectionItemDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateInspectionReq {
    /// `move_in` | `move_out`.
    pub kind: String,
    /// ISO date (`YYYY-MM-DD`).
    pub scheduled_date: Option<String>,
    pub notes: Option<String>,
    /// Skip generating the default checklist (default false).
    pub blank: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateInspectionReq {
    pub scheduled_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AddInspectionItemReq {
    pub area: String,
    pub item: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateInspectionItemReq {
    /// `unrated` | `good` | `fair` | `poor` | `damaged`.
    pub condition: Option<String>,
    pub notes: Option<String>,
}

// ---------------------------------------------------------------------------
// Deposit disposition
// ---------------------------------------------------------------------------

#[derive(Serialize, schemars::JsonSchema)]
pub struct DeductionDto {
    pub id: Uuid,
    pub description: String,
    pub amount_cents: i64,
    pub amount_label: String,
}

impl From<entity::deposit_deduction::Model> for DeductionDto {
    fn from(d: entity::deposit_deduction::Model) -> Self {
        DeductionDto {
            id: d.id,
            description: d.description,
            amount_label: usd(d.amount_cents),
            amount_cents: d.amount_cents,
        }
    }
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct DispositionDto {
    pub id: Uuid,
    pub lease_id: Uuid,
    pub property_id: Uuid,
    /// `draft` | `processing` | `closed` | `failed`.
    pub status: String,
    pub deposit_cents: i64,
    pub deposit_label: String,
    pub withheld_cents: i64,
    pub withheld_label: String,
    pub refund_cents: Option<i64>,
    pub refund_label: Option<String>,
    pub notes: Option<String>,
    pub failure_reason: Option<String>,
    pub statement_document_id: Option<Uuid>,
    pub deductions: Vec<DeductionDto>,
    pub finalized_at: Option<String>,
    pub closed_at: Option<String>,
    pub created_at: String,
}

pub fn disposition_dto(
    d: entity::deposit_disposition::Model,
    deductions: Vec<entity::deposit_deduction::Model>,
) -> DispositionDto {
    let withheld: i64 = deductions.iter().map(|x| x.amount_cents).sum();
    DispositionDto {
        id: d.id,
        lease_id: d.lease_id,
        property_id: d.property_id,
        status: d.status,
        deposit_label: usd(d.deposit_cents),
        deposit_cents: d.deposit_cents,
        withheld_label: usd(withheld),
        withheld_cents: withheld,
        refund_label: d.refund_cents.map(usd),
        refund_cents: d.refund_cents,
        notes: d.notes,
        failure_reason: d.failure_reason,
        statement_document_id: d.statement_document_id,
        deductions: deductions.into_iter().map(DeductionDto::from).collect(),
        finalized_at: d.finalized_at.map(|t| t.to_rfc3339()),
        closed_at: d.closed_at.map(|t| t.to_rfc3339()),
        created_at: d.created_at.to_rfc3339(),
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DeductionInput {
    pub description: String,
    pub amount_cents: i64,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpsertDispositionReq {
    pub deductions: Vec<DeductionInput>,
    pub notes: Option<String>,
}

/// The lease's deposit picture for the console + portal.
#[derive(Serialize, schemars::JsonSchema)]
pub struct LeaseDepositResp {
    pub lease_id: Uuid,
    pub deposit_cents: Option<i64>,
    pub deposit_label: Option<String>,
    /// The deposit has a settled payment in trust.
    pub deposit_paid: bool,
    pub disposition: Option<DispositionDto>,
}
