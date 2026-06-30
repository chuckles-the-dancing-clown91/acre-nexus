//! **Tenant history** — a landlord/back-office view of who has rented, past and
//! present, aggregated across leases. Residents are grouped by email (falling back
//! to name), each with their full tenancy timeline, current standing, and whether
//! the tenancy came from an application. Gated by `lease:read` (held by landlords
//! and back-office staff).

pub mod dto;
pub mod list;
pub mod property;

use dto::{TenancySummary, TenantHistoryRow};
use std::collections::HashMap;

/// Group leases into per-resident history rows. `prop_names` maps property id →
/// display name. Tenancies are newest-first; rows with an active tenancy sort
/// first, then by most recent move-in.
pub fn build_history(
    leases: Vec<entity::lease::Model>,
    prop_names: &HashMap<uuid::Uuid, String>,
) -> Vec<TenantHistoryRow> {
    let mut groups: HashMap<String, Vec<entity::lease::Model>> = HashMap::new();
    for l in leases {
        let key = l
            .tenant_email
            .clone()
            .map(|e| e.to_lowercase())
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| l.tenant_name.to_lowercase());
        groups.entry(key).or_default().push(l);
    }

    let mut rows: Vec<TenantHistoryRow> = groups
        .into_values()
        .map(|mut ls| {
            ls.sort_by(|a, b| b.start_date.cmp(&a.start_date));
            let first = &ls[0];
            let current = ls.iter().any(|l| l.status == "active");
            let latest_start = first.start_date.clone();
            TenantHistoryRow {
                tenant_name: first.tenant_name.clone(),
                tenant_email: first.tenant_email.clone(),
                tenant_phone: first.tenant_phone.clone(),
                current,
                lease_count: ls.len(),
                latest_start,
                tenancies: ls
                    .into_iter()
                    .map(|l| TenancySummary::from_lease(l, prop_names))
                    .collect(),
            }
        })
        .collect();

    rows.sort_by(|a, b| {
        b.current
            .cmp(&a.current)
            .then(b.latest_start.cmp(&a.latest_start))
    });
    rows
}
