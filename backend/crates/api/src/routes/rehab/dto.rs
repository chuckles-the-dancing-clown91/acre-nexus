//! DTOs for the rehab / construction domain (issue #40).

use super::waiver_type_label;
use crate::dto::usd;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct RehabLineDto {
    pub id: Uuid,
    pub category: String,
    pub description: Option<String>,
    pub budget_cents: i64,
    pub budget_label: String,
    pub sort_order: i32,
}

impl From<entity::rehab_line::Model> for RehabLineDto {
    fn from(l: entity::rehab_line::Model) -> Self {
        RehabLineDto {
            budget_label: usd(l.budget_cents),
            id: l.id,
            category: l.category,
            description: l.description,
            budget_cents: l.budget_cents,
            sort_order: l.sort_order,
        }
    }
}

#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct ChangeOrderDto {
    pub id: Uuid,
    pub description: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub status: String,
    pub created_at: String,
    pub decided_at: Option<String>,
}

impl From<entity::rehab_change_order::Model> for ChangeOrderDto {
    fn from(c: entity::rehab_change_order::Model) -> Self {
        ChangeOrderDto {
            amount_label: usd(c.amount_cents),
            id: c.id,
            description: c.description,
            amount_cents: c.amount_cents,
            status: c.status,
            created_at: c.created_at.to_rfc3339(),
            decided_at: c.decided_at.map(|d| d.to_rfc3339()),
        }
    }
}

#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct RehabDrawDto {
    pub id: Uuid,
    pub project_id: Uuid,
    pub number: i32,
    pub title: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub status: String,
    pub contractor_id: Option<Uuid>,
    pub contractor_name: Option<String>,
    pub notes: Option<String>,
    pub funded_at: Option<String>,
    pub created_at: String,
}

impl RehabDrawDto {
    pub fn build(d: &entity::rehab_draw::Model, contractor_name: Option<String>) -> Self {
        RehabDrawDto {
            id: d.id,
            project_id: d.project_id,
            number: d.number,
            title: d.title.clone(),
            amount_cents: d.amount_cents,
            amount_label: usd(d.amount_cents),
            status: d.status.clone(),
            contractor_id: d.contractor_id,
            contractor_name,
            notes: d.notes.clone(),
            funded_at: d.funded_at.map(|t| t.to_rfc3339()),
            created_at: d.created_at.to_rfc3339(),
        }
    }
}

#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct LienWaiverDto {
    pub id: Uuid,
    pub draw_id: Uuid,
    pub waiver_type: String,
    pub waiver_type_label: String,
    pub contractor_name: String,
    pub amount_cents: i64,
    pub amount_label: String,
    pub through_date: Option<String>,
    pub status: String,
    pub document_id: Option<Uuid>,
    pub created_at: String,
}

impl From<entity::rehab_lien_waiver::Model> for LienWaiverDto {
    fn from(w: entity::rehab_lien_waiver::Model) -> Self {
        LienWaiverDto {
            waiver_type_label: waiver_type_label(&w.waiver_type).to_string(),
            amount_label: usd(w.amount_cents),
            id: w.id,
            draw_id: w.draw_id,
            waiver_type: w.waiver_type,
            contractor_name: w.contractor_name,
            amount_cents: w.amount_cents,
            through_date: w.through_date,
            status: w.status,
            document_id: w.document_id,
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

/// A rehab project with its computed budget roll-up.
#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct RehabProjectDto {
    pub id: Uuid,
    pub property_id: Uuid,
    pub name: String,
    pub status: String,
    pub base_budget_cents: i64,
    pub base_budget_label: String,
    pub contingency_bps: i32,
    pub contingency_pct: f64,
    pub contingency_cents: i64,
    pub contingency_label: String,
    /// Base budget + approved change orders.
    pub adjusted_budget_cents: i64,
    pub adjusted_budget_label: String,
    pub approved_change_orders_cents: i64,
    pub approved_change_orders_label: String,
    /// Sum of funded draws.
    pub drawn_cents: i64,
    pub drawn_label: String,
    /// Sum of requested / approved (not yet funded) draws.
    pub pending_draws_cents: i64,
    pub pending_draws_label: String,
    /// Adjusted budget − drawn.
    pub remaining_cents: i64,
    pub remaining_label: String,
    /// Sum of the itemised scope lines.
    pub lines_budget_cents: i64,
    pub lines_budget_label: String,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub notes: Option<String>,
    pub line_count: i32,
    pub draw_count: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl RehabProjectDto {
    pub fn build(
        p: &entity::rehab_project::Model,
        lines: &[entity::rehab_line::Model],
        draws: &[entity::rehab_draw::Model],
        change_orders: &[entity::rehab_change_order::Model],
    ) -> Self {
        let approved_co: i64 = change_orders
            .iter()
            .filter(|c| c.status == "approved")
            .map(|c| c.amount_cents)
            .sum();
        let adjusted = p.budget_cents + approved_co;
        let drawn: i64 = draws
            .iter()
            .filter(|d| d.status == "funded")
            .map(|d| d.amount_cents)
            .sum();
        let pending: i64 = draws
            .iter()
            .filter(|d| d.status == "requested" || d.status == "approved")
            .map(|d| d.amount_cents)
            .sum();
        let lines_budget: i64 = lines.iter().map(|l| l.budget_cents).sum();
        let contingency = (p.budget_cents as i128 * p.contingency_bps as i128 / 10_000) as i64;

        RehabProjectDto {
            id: p.id,
            property_id: p.property_id,
            name: p.name.clone(),
            status: p.status.clone(),
            base_budget_cents: p.budget_cents,
            base_budget_label: usd(p.budget_cents),
            contingency_bps: p.contingency_bps,
            contingency_pct: p.contingency_bps as f64 / 100.0,
            contingency_cents: contingency,
            contingency_label: usd(contingency),
            adjusted_budget_cents: adjusted,
            adjusted_budget_label: usd(adjusted),
            approved_change_orders_cents: approved_co,
            approved_change_orders_label: usd(approved_co),
            drawn_cents: drawn,
            drawn_label: usd(drawn),
            pending_draws_cents: pending,
            pending_draws_label: usd(pending),
            remaining_cents: adjusted - drawn,
            remaining_label: usd(adjusted - drawn),
            lines_budget_cents: lines_budget,
            lines_budget_label: usd(lines_budget),
            start_date: p.start_date.clone(),
            target_end_date: p.target_end_date.clone(),
            notes: p.notes.clone(),
            line_count: lines.len() as i32,
            draw_count: draws.len() as i32,
            created_at: p.created_at.to_rfc3339(),
            updated_at: p.updated_at.to_rfc3339(),
        }
    }
}

/// Full project detail: the roll-up plus lines, draws, and change orders.
#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct RehabProjectDetailDto {
    #[serde(flatten)]
    pub project: RehabProjectDto,
    pub lines: Vec<RehabLineDto>,
    pub draws: Vec<RehabDrawDto>,
    pub change_orders: Vec<ChangeOrderDto>,
}

/// Draw detail: the draw plus its lien waivers.
#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct RehabDrawDetailDto {
    #[serde(flatten)]
    pub draw: RehabDrawDto,
    pub lien_waivers: Vec<LienWaiverDto>,
}

/// Resolve counterparty names for a set of ids (best-effort; missing → absent).
pub fn contractor_names(rows: &[entity::counterparty::Model]) -> HashMap<Uuid, String> {
    rows.iter().map(|c| (c.id, c.name.clone())).collect()
}

// ---- request bodies ----

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateProjectReq {
    pub name: String,
    #[serde(default)]
    pub budget_cents: Option<i64>,
    #[serde(default)]
    pub contingency_bps: Option<i32>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub target_end_date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct UpdateProjectReq {
    pub name: Option<String>,
    pub status: Option<String>,
    pub budget_cents: Option<i64>,
    pub contingency_bps: Option<i32>,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateLineReq {
    pub category: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub budget_cents: Option<i64>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct UpdateLineReq {
    pub category: Option<String>,
    pub description: Option<String>,
    pub budget_cents: Option<i64>,
    pub sort_order: Option<i32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateChangeOrderReq {
    pub description: String,
    pub amount_cents: i64,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DecideReq {
    pub approve: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateDrawReq {
    pub title: String,
    pub amount_cents: i64,
    #[serde(default)]
    pub contractor_id: Option<Uuid>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DrawStatusReq {
    /// `requested` | `approved` | `funded` | `rejected`.
    pub status: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateLienWaiverReq {
    pub waiver_type: String,
    #[serde(default)]
    pub contractor_id: Option<Uuid>,
    #[serde(default)]
    pub contractor_name: Option<String>,
    #[serde(default)]
    pub amount_cents: Option<i64>,
    #[serde(default)]
    pub through_date: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateLienWaiverReq {
    /// `generated` | `received`.
    pub status: String,
}
