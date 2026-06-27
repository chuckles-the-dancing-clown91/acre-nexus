//! Request/response shapes for the title & ownership endpoints (ownership
//! records and liens recorded against a property's title).

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Label an optional cents amount as USD.
fn label(cents: Option<i64>) -> Option<String> {
    cents.map(usd)
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct OwnershipDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub owner_kind: String,
    pub owner_id: Option<Uuid>,
    pub owner_name: String,
    pub vesting: Option<String>,
    pub percent_bps: i32,
    pub deed_type: Option<String>,
    pub deed_recorded_date: Option<String>,
    pub deed_reference: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::ownership::Model> for OwnershipDto {
    fn from(o: entity::ownership::Model) -> Self {
        OwnershipDto {
            id: o.id,
            tenant_id: o.tenant_id,
            property_id: o.property_id,
            owner_kind: o.owner_kind,
            owner_id: o.owner_id,
            owner_name: o.owner_name,
            vesting: o.vesting,
            percent_bps: o.percent_bps,
            deed_type: o.deed_type,
            deed_recorded_date: o.deed_recorded_date,
            deed_reference: o.deed_reference,
            created_at: o.created_at.to_rfc3339(),
            updated_at: o.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateOwnershipReq {
    pub owner_kind: Option<String>,
    pub owner_id: Option<Uuid>,
    pub owner_name: String,
    pub vesting: Option<String>,
    pub percent_bps: Option<i32>,
    pub deed_type: Option<String>,
    pub deed_recorded_date: Option<String>,
    pub deed_reference: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateOwnershipReq {
    pub owner_kind: Option<String>,
    pub owner_id: Option<Uuid>,
    pub owner_name: Option<String>,
    pub vesting: Option<String>,
    pub percent_bps: Option<i32>,
    pub deed_type: Option<String>,
    pub deed_recorded_date: Option<String>,
    pub deed_reference: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LienDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub lienholder_id: Option<Uuid>,
    pub lienholder_name: String,
    pub kind: String,
    pub amount_cents: Option<i64>,
    pub amount_label: Option<String>,
    pub position: Option<i32>,
    pub recorded_date: Option<String>,
    pub status: String,
    pub reference: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::lien::Model> for LienDto {
    fn from(l: entity::lien::Model) -> Self {
        LienDto {
            amount_label: label(l.amount_cents),
            id: l.id,
            tenant_id: l.tenant_id,
            property_id: l.property_id,
            lienholder_id: l.lienholder_id,
            lienholder_name: l.lienholder_name,
            kind: l.kind,
            amount_cents: l.amount_cents,
            position: l.position,
            recorded_date: l.recorded_date,
            status: l.status,
            reference: l.reference,
            notes: l.notes,
            created_at: l.created_at.to_rfc3339(),
            updated_at: l.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateLienReq {
    pub lienholder_id: Option<Uuid>,
    pub lienholder_name: String,
    pub kind: Option<String>,
    pub amount_cents: Option<i64>,
    pub position: Option<i32>,
    pub recorded_date: Option<String>,
    pub status: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateLienReq {
    pub lienholder_id: Option<Uuid>,
    pub lienholder_name: Option<String>,
    pub kind: Option<String>,
    pub amount_cents: Option<i64>,
    pub position: Option<i32>,
    pub recorded_date: Option<String>,
    pub status: Option<String>,
    pub reference: Option<String>,
    pub notes: Option<String>,
}
