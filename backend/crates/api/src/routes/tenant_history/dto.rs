use crate::dto::usd;
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

/// One tenancy (lease) in a resident's history.
#[derive(Serialize, schemars::JsonSchema)]
pub struct TenancySummary {
    pub lease_id: Uuid,
    pub property_id: Uuid,
    pub property_name: Option<String>,
    pub unit_id: Option<Uuid>,
    pub status: String,
    pub payment_status: String,
    pub start_date: String,
    pub end_date: Option<String>,
    pub rent_cents: i64,
    pub rent_label: String,
    pub balance_cents: i64,
    pub balance_label: String,
    /// Whether this lease originated from a rental application.
    pub from_application: bool,
}

impl TenancySummary {
    pub fn from_lease(l: entity::lease::Model, prop_names: &HashMap<Uuid, String>) -> Self {
        TenancySummary {
            property_name: prop_names.get(&l.property_id).cloned(),
            lease_id: l.id,
            property_id: l.property_id,
            unit_id: l.unit_id,
            status: l.status,
            payment_status: l.payment_status,
            start_date: l.start_date,
            end_date: l.end_date,
            rent_cents: l.rent_cents,
            rent_label: usd(l.rent_cents),
            balance_cents: l.balance_cents,
            balance_label: usd(l.balance_cents),
            from_application: l.application_id.is_some(),
        }
    }
}

/// A resident and their full tenancy history.
#[derive(Serialize, schemars::JsonSchema)]
pub struct TenantHistoryRow {
    pub tenant_name: String,
    pub tenant_email: Option<String>,
    pub tenant_phone: Option<String>,
    /// True when the resident has a currently-active tenancy.
    pub current: bool,
    pub lease_count: usize,
    /// Most recent move-in date (drives ordering).
    pub latest_start: String,
    pub tenancies: Vec<TenancySummary>,
}
