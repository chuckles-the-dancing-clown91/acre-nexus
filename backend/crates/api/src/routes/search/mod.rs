//! **Global search** (roadmap Phase 8, issue #55) — one box that finds a
//! property, tenant, counterparty, maintenance ticket, or LLC by name / address
//! / email, without navigating into a module first.
//!
//! Tenant-scoped (never crosses tenants) and **permission-aware**: a result type
//! only appears when the caller holds its read permission. Matching is
//! case-insensitive substring over the tenant's own rows — the same
//! `.all(&db)` shape the list endpoints already use; a trigram/tsvector index is
//! the natural optimisation once a tenant's data outgrows this.

use crate::auth::AuthUser;
use crate::error::ApiResult;
use crate::rbac::Permission;
use crate::state::AppState;
use crate::tenancy::TenantScope;
use entity::prelude::{Counterparty, Lease, Llc, MaintenanceTicket, Property};
use rocket::serde::json::Json;
use rocket::{get, State};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::HashMap;
use uuid::Uuid;

/// Max hits returned per result type.
const PER_TYPE: usize = 6;

#[derive(Serialize, schemars::JsonSchema)]
pub struct SearchHit {
    /// `property` | `lease` | `entity` | `ticket` | `llc`.
    pub kind: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    /// Console path this hit links to.
    pub href: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub struct SearchResp {
    pub query: String,
    pub hits: Vec<SearchHit>,
}

/// Case-insensitive substring match.
fn hit(hay: &str, needle: &str) -> bool {
    hay.to_lowercase().contains(needle)
}

/// Rank: a title that *starts with* the query sorts before a mid-string match,
/// then alphabetically. Applied per type before truncating to [`PER_TYPE`].
fn rank_and_take(mut hits: Vec<(bool, SearchHit)>) -> Vec<SearchHit> {
    hits.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.title.to_lowercase().cmp(&b.1.title.to_lowercase()))
    });
    hits.into_iter().take(PER_TYPE).map(|(_, h)| h).collect()
}

/// `GET /search?<q>&<limit>` — grouped global search across the tenant's data.
#[rocket_okapi::openapi(tag = "Search")]
#[get("/search?<q>")]
pub async fn search(
    state: &State<AppState>,
    db: crate::db::RequestDb,
    user: AuthUser,
    scope: TenantScope,
    q: Option<String>,
) -> ApiResult<Json<SearchResp>> {
    crate::modules::require_enabled(&state.db, scope.tenant_id, "search").await?;
    let query = q.unwrap_or_default().trim().to_string();
    let ql = query.to_lowercase();
    if ql.len() < 2 {
        return Ok(Json(SearchResp {
            query,
            hits: vec![],
        }));
    }

    let can = |p: Permission| user.require(p).is_ok();
    let mut hits: Vec<SearchHit> = Vec::new();

    // Properties (also used to resolve lease/ticket property names).
    let properties = Property::find()
        .filter(entity::property::Column::TenantId.eq(scope.tenant_id))
        .all(&db)
        .await?;
    let prop_name: HashMap<Uuid, String> =
        properties.iter().map(|p| (p.id, p.name.clone())).collect();

    if can(Permission::PropertyRead) {
        let matched = properties
            .iter()
            .filter(|p| hit(&p.name, &ql) || hit(&p.address, &ql) || hit(&p.city, &ql))
            .map(|p| {
                (
                    p.name.to_lowercase().starts_with(&ql),
                    SearchHit {
                        kind: "property".into(),
                        id: p.id.to_string(),
                        title: p.name.clone(),
                        subtitle: format!("{}, {}", p.address, p.city),
                        href: format!("/console/properties/{}", p.id),
                    },
                )
            })
            .collect();
        hits.extend(rank_and_take(matched));
    }

    // Tenants (via their lease).
    if can(Permission::LeaseRead) {
        let leases = Lease::find()
            .filter(entity::lease::Column::TenantId.eq(scope.tenant_id))
            .all(&db)
            .await?;
        let matched = leases
            .iter()
            .filter(|l| {
                hit(&l.tenant_name, &ql) || l.tenant_email.as_deref().is_some_and(|e| hit(e, &ql))
            })
            .map(|l| {
                let prop = prop_name.get(&l.property_id).cloned().unwrap_or_default();
                (
                    l.tenant_name.to_lowercase().starts_with(&ql),
                    SearchHit {
                        kind: "lease".into(),
                        id: l.id.to_string(),
                        title: l.tenant_name.clone(),
                        subtitle: format!("Tenant · {prop}"),
                        href: format!("/console/properties/{}/tenants", l.property_id),
                    },
                )
            })
            .collect();
        hits.extend(rank_and_take(matched));
    }

    // Counterparties (entities registry).
    if can(Permission::EntityRead) {
        let rows = Counterparty::find()
            .filter(entity::counterparty::Column::TenantId.eq(scope.tenant_id))
            .all(&db)
            .await?;
        let matched = rows
            .iter()
            .filter(|c| {
                hit(&c.name, &ql)
                    || c.email.as_deref().is_some_and(|e| hit(e, &ql))
                    || c.contact_name.as_deref().is_some_and(|n| hit(n, &ql))
            })
            .map(|c| {
                (
                    c.name.to_lowercase().starts_with(&ql),
                    SearchHit {
                        kind: "entity".into(),
                        id: c.id.to_string(),
                        title: c.name.clone(),
                        subtitle: format!("{} · contact", c.kind),
                        href: "/console/entities".into(),
                    },
                )
            })
            .collect();
        hits.extend(rank_and_take(matched));
    }

    // Maintenance tickets.
    if can(Permission::MaintenanceRead) {
        let rows = MaintenanceTicket::find()
            .filter(entity::maintenance_ticket::Column::TenantId.eq(scope.tenant_id))
            .all(&db)
            .await?;
        let matched = rows
            .iter()
            .filter(|t| hit(&t.title, &ql) || t.description.as_deref().is_some_and(|d| hit(d, &ql)))
            .map(|t| {
                let prop = prop_name.get(&t.property_id).cloned().unwrap_or_default();
                (
                    t.title.to_lowercase().starts_with(&ql),
                    SearchHit {
                        kind: "ticket".into(),
                        id: t.id.to_string(),
                        title: t.title.clone(),
                        subtitle: format!("Ticket · {} · {prop}", t.status),
                        href: format!("/console/maintenance/{}", t.id),
                    },
                )
            })
            .collect();
        hits.extend(rank_and_take(matched));
    }

    // LLCs (holding entities).
    if can(Permission::PropertyRead) {
        let rows = Llc::find()
            .filter(entity::llc::Column::TenantId.eq(scope.tenant_id))
            .all(&db)
            .await?;
        let matched = rows
            .iter()
            .filter(|l| hit(&l.name, &ql))
            .map(|l| {
                (
                    l.name.to_lowercase().starts_with(&ql),
                    SearchHit {
                        kind: "llc".into(),
                        id: l.id.to_string(),
                        title: l.name.clone(),
                        subtitle: "Legal entity".into(),
                        href: "/console/llcs".into(),
                    },
                )
            })
            .collect();
        hits.extend(rank_and_take(matched));
    }

    Ok(Json(SearchResp { query, hits }))
}
