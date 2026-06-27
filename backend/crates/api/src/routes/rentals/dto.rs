//! Request/response shapes for the rentals & leasing endpoints (units, leases,
//! and the lease payment ledger), with USD labels for display.

use crate::dto::usd;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Label an optional cents amount as USD.
fn label(cents: Option<i64>) -> Option<String> {
    cents.map(usd)
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct UnitDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_number: String,
    pub beds: Option<i32>,
    pub baths: Option<f64>,
    pub sqft: Option<i32>,
    pub market_rent_cents: Option<i64>,
    pub market_rent_label: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::unit::Model> for UnitDto {
    fn from(u: entity::unit::Model) -> Self {
        UnitDto {
            market_rent_label: label(u.market_rent_cents),
            id: u.id,
            tenant_id: u.tenant_id,
            property_id: u.property_id,
            unit_number: u.unit_number,
            beds: u.beds,
            baths: u.baths,
            sqft: u.sqft,
            market_rent_cents: u.market_rent_cents,
            status: u.status,
            created_at: u.created_at.to_rfc3339(),
            updated_at: u.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateUnitReq {
    pub unit_number: String,
    pub beds: Option<i32>,
    pub baths: Option<f64>,
    pub sqft: Option<i32>,
    pub market_rent_cents: Option<i64>,
    pub status: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateUnitReq {
    pub unit_number: Option<String>,
    pub beds: Option<i32>,
    pub baths: Option<f64>,
    pub sqft: Option<i32>,
    pub market_rent_cents: Option<i64>,
    pub status: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LeaseDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub property_id: Uuid,
    pub unit_id: Option<Uuid>,
    pub tenant_name: String,
    pub tenant_email: Option<String>,
    pub tenant_phone: Option<String>,
    pub rent_cents: i64,
    pub rent_label: Option<String>,
    pub deposit_cents: Option<i64>,
    pub deposit_label: Option<String>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub status: String,
    pub payment_status: String,
    pub balance_cents: i64,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<entity::lease::Model> for LeaseDto {
    fn from(l: entity::lease::Model) -> Self {
        LeaseDto {
            rent_label: label(Some(l.rent_cents)),
            deposit_label: label(l.deposit_cents),
            id: l.id,
            tenant_id: l.tenant_id,
            property_id: l.property_id,
            unit_id: l.unit_id,
            tenant_name: l.tenant_name,
            tenant_email: l.tenant_email,
            tenant_phone: l.tenant_phone,
            rent_cents: l.rent_cents,
            deposit_cents: l.deposit_cents,
            start_date: l.start_date,
            end_date: l.end_date,
            status: l.status,
            payment_status: l.payment_status,
            balance_cents: l.balance_cents,
            notes: l.notes,
            created_at: l.created_at.to_rfc3339(),
            updated_at: l.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateLeaseReq {
    pub unit_id: Option<Uuid>,
    pub tenant_name: String,
    pub tenant_email: Option<String>,
    pub tenant_phone: Option<String>,
    pub rent_cents: i64,
    pub deposit_cents: Option<i64>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub status: Option<String>,
    pub payment_status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateLeaseReq {
    pub unit_id: Option<Uuid>,
    pub tenant_name: Option<String>,
    pub tenant_email: Option<String>,
    pub tenant_phone: Option<String>,
    pub rent_cents: Option<i64>,
    pub deposit_cents: Option<i64>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub status: Option<String>,
    pub payment_status: Option<String>,
    pub balance_cents: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct LeasePaymentDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub lease_id: Uuid,
    pub due_date: String,
    pub amount_cents: i64,
    pub amount_label: Option<String>,
    pub paid_date: Option<String>,
    pub status: String,
    pub method: Option<String>,
    pub created_at: String,
}

impl From<entity::lease_payment::Model> for LeasePaymentDto {
    fn from(p: entity::lease_payment::Model) -> Self {
        LeasePaymentDto {
            amount_label: label(Some(p.amount_cents)),
            id: p.id,
            tenant_id: p.tenant_id,
            lease_id: p.lease_id,
            due_date: p.due_date,
            amount_cents: p.amount_cents,
            paid_date: p.paid_date,
            status: p.status,
            method: p.method,
            created_at: p.created_at.to_rfc3339(),
        }
    }
}

/// A lease with its full payment ledger, for the lease detail page.
#[derive(Serialize, schemars::JsonSchema)]
pub struct LeaseDetailDto {
    #[serde(flatten)]
    pub lease: LeaseDto,
    pub payments: Vec<LeasePaymentDto>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RecordPaymentReq {
    pub due_date: String,
    pub amount_cents: i64,
    pub paid_date: Option<String>,
    pub status: Option<String>,
    pub method: Option<String>,
}
