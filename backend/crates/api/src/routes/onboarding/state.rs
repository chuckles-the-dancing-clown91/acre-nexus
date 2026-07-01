//! The per-tenant **onboarding state machine** (§9). Each step has a completion
//! predicate evaluated against the live database, so the workflow is resumable
//! from any incomplete step and never drifts from reality. Optional steps don't
//! block `live` but surface as setup nudges.
//!
//! ```text
//! provisioning → firm_admin_accepted → branding_configured → domains_configured
//!   → entities_created → banking_linked → portfolio_imported → staff_invited → live
//! ```

use crate::error::ApiResult;
use entity::prelude::{BankAccount, Domain, Llc, Membership, Property, Theme};
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use std::collections::HashSet;
use uuid::Uuid;

/// One onboarding step's computed status.
#[derive(Serialize, schemars::JsonSchema)]
pub struct StepStatus {
    pub key: &'static str,
    pub label: &'static str,
    pub complete: bool,
    /// Optional steps don't block reaching `live`.
    pub optional: bool,
}

/// The full computed workflow snapshot.
#[derive(Serialize, schemars::JsonSchema)]
pub struct WorkflowSnapshot {
    /// Furthest-reached state key (or `live`).
    pub state: String,
    pub steps: Vec<StepStatus>,
    /// Whether all *required* steps are complete.
    pub live: bool,
}

struct StepDef {
    key: &'static str,
    label: &'static str,
    optional: bool,
}

/// Ordered steps after `provisioning`. `domains_configured` and `staff_invited`
/// are optional (don't block `live`).
const STEPS: &[StepDef] = &[
    StepDef {
        key: "firm_admin_accepted",
        label: "Firm owner active",
        optional: false,
    },
    StepDef {
        key: "branding_configured",
        label: "Branding configured",
        optional: false,
    },
    StepDef {
        key: "domains_configured",
        label: "Domains configured",
        optional: true,
    },
    StepDef {
        key: "entities_created",
        label: "Legal entities created",
        optional: false,
    },
    StepDef {
        key: "banking_linked",
        label: "Bank & trust accounts linked",
        optional: false,
    },
    StepDef {
        key: "portfolio_imported",
        label: "Portfolio imported",
        optional: false,
    },
    StepDef {
        key: "staff_invited",
        label: "Staff invited",
        optional: true,
    },
];

/// Evaluate every step predicate for `tenant_id` and derive the state.
pub async fn compute(db: &impl ConnectionTrait, tenant_id: Uuid) -> ApiResult<WorkflowSnapshot> {
    // ---- gather the facts each predicate needs ----
    let memberships = Membership::find()
        .filter(entity::membership::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let owner_active = memberships
        .iter()
        .any(|m| m.profile_type == "tenant_owner" && m.status == "active");
    let extra_staff = memberships
        .iter()
        .filter(|m| m.status == "active" && m.profile_type != "tenant_owner")
        .count()
        > 0;

    let branding = Theme::find()
        .filter(entity::theme::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .map(|t| !t.company_name.trim().is_empty())
        .unwrap_or(false);

    let domains_ok = Domain::find()
        .filter(entity::domain::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .iter()
        .any(|d| d.verified_at.is_some() && d.tls_status == "active");

    let llcs = Llc::find()
        .filter(entity::llc::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let entities_ok = !llcs.is_empty();

    // banking_linked: every LLC has ≥1 operating and ≥1 trust account.
    let accounts = BankAccount::find()
        .filter(entity::bank_account::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?;
    let has_operating: HashSet<Uuid> = accounts
        .iter()
        .filter(|a| a.kind == "operating")
        .map(|a| a.entity_id)
        .collect();
    let has_trust: HashSet<Uuid> = accounts
        .iter()
        .filter(|a| a.kind == "trust")
        .map(|a| a.entity_id)
        .collect();
    let banking_ok = entities_ok
        && llcs
            .iter()
            .all(|l| has_operating.contains(&l.id) && has_trust.contains(&l.id));

    let portfolio_ok = Property::find()
        .filter(entity::property::Column::TenantId.eq(tenant_id))
        .all(db)
        .await?
        .iter()
        .any(|p| p.llc_id.is_some());

    // ---- map predicates to step keys ----
    let complete = |key: &str| -> bool {
        match key {
            "firm_admin_accepted" => owner_active,
            "branding_configured" => branding,
            "domains_configured" => domains_ok,
            "entities_created" => entities_ok,
            "banking_linked" => banking_ok,
            "portfolio_imported" => portfolio_ok,
            "staff_invited" => extra_staff,
            _ => false,
        }
    };

    let steps: Vec<StepStatus> = STEPS
        .iter()
        .map(|s| StepStatus {
            key: s.key,
            label: s.label,
            complete: complete(s.key),
            optional: s.optional,
        })
        .collect();

    let live = steps.iter().filter(|s| !s.optional).all(|s| s.complete);
    let state_key = if live {
        "live".to_string()
    } else {
        // Furthest step reached: the last contiguous-or-any completed step key,
        // else still provisioning.
        steps
            .iter()
            .rev()
            .find(|s| s.complete)
            .map(|s| s.key.to_string())
            .unwrap_or_else(|| "provisioning".to_string())
    };

    Ok(WorkflowSnapshot {
        state: state_key,
        steps,
        live,
    })
}
