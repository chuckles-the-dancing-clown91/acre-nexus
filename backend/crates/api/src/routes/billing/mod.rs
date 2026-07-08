//! **SaaS billing — workspace self-serve** (roadmap Phase 8). The client-facing
//! half of platform billing: a workspace views its current subscription (plan,
//! live unit meter, and the estimated charge for the period in progress) and its
//! platform invoice history, gated by `billing:read`. The platform-plane half —
//! metering, generating, and settling invoices across every workspace — lives in
//! [`crate::routes::platform::billing`]; the pricing engine in [`crate::saas`].

pub mod invoices;
pub mod subscription;

use crate::dto::usd;
use crate::saas::{self, Plan};
use serde::Serialize;
use uuid::Uuid;

/// A published plan, as shown on a pricing / plan-picker card.
#[derive(Serialize, schemars::JsonSchema)]
pub struct PlanDto {
    pub key: String,
    pub name: String,
    pub base_cents: i64,
    pub base_label: String,
    pub included_units: i32,
    pub overage_cents: i64,
    pub overage_label: String,
    pub blurb: String,
    pub features: Vec<String>,
    /// True for the workspace's current plan.
    pub current: bool,
}

impl PlanDto {
    pub fn from(plan: &Plan, current_key: &str) -> Self {
        PlanDto {
            key: plan.key.into(),
            name: plan.name.into(),
            base_cents: plan.base_cents,
            base_label: format!("{}/mo", usd(plan.base_cents)),
            included_units: plan.included_units,
            overage_cents: plan.overage_cents,
            overage_label: format!("{}/unit", usd(plan.overage_cents)),
            blurb: plan.blurb.into(),
            features: plan.features.iter().map(|f| f.to_string()).collect(),
            current: plan.key == current_key,
        }
    }
}

/// One invoice line, formatted for display.
#[derive(Serialize, schemars::JsonSchema)]
pub struct LineDto {
    pub description: String,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub unit_price_label: String,
    pub amount_cents: i64,
    pub amount_label: String,
}

impl LineDto {
    fn from(m: entity::platform_invoice_line::Model) -> Self {
        LineDto {
            description: m.description,
            quantity: m.quantity,
            unit_price_cents: m.unit_price_cents,
            unit_price_label: usd(m.unit_price_cents),
            amount_cents: m.amount_cents,
            amount_label: usd(m.amount_cents),
        }
    }
}

/// A platform invoice with its lines, formatted for display.
#[derive(Serialize, schemars::JsonSchema)]
pub struct InvoiceDto {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub period: String,
    pub plan: String,
    pub status: String,
    pub unit_count: i32,
    pub included_units: i32,
    pub base_cents: i64,
    pub base_label: String,
    pub overage_cents: i64,
    pub overage_label: String,
    pub total_cents: i64,
    pub total_label: String,
    pub issued_at: Option<String>,
    pub due_date: Option<String>,
    pub paid_at: Option<String>,
    pub lines: Vec<LineDto>,
}

impl InvoiceDto {
    pub fn from(
        inv: entity::platform_invoice::Model,
        lines: Vec<entity::platform_invoice_line::Model>,
    ) -> Self {
        InvoiceDto {
            id: inv.id,
            tenant_id: inv.tenant_id,
            period: inv.period,
            plan: inv.plan,
            status: inv.status,
            unit_count: inv.unit_count,
            included_units: inv.included_units,
            base_cents: inv.base_cents,
            base_label: usd(inv.base_cents),
            overage_cents: inv.overage_cents,
            overage_label: usd(inv.overage_cents),
            total_cents: inv.total_cents,
            total_label: usd(inv.total_cents),
            issued_at: inv.issued_at.map(|t| t.to_rfc3339()),
            due_date: inv.due_date,
            paid_at: inv.paid_at.map(|t| t.to_rfc3339()),
            lines: lines.into_iter().map(LineDto::from).collect(),
        }
    }
}

/// Human month label from a `YYYY-MM` period, e.g. `2026-06` → `June 2026`.
pub fn period_label(period: &str) -> String {
    let parts: Vec<&str> = period.split('-').collect();
    if let [y, m] = parts[..] {
        if let (Ok(year), Ok(month)) = (y.parse::<i32>(), m.parse::<u32>()) {
            if let Some(d) = chrono::NaiveDate::from_ymd_opt(year, month, 1) {
                return d.format("%B %Y").to_string();
            }
        }
    }
    period.to_string()
}

/// Render an invoice to the shared report table (for CSV / PDF export).
pub fn invoice_table(inv: &InvoiceDto) -> crate::routes::reports::ReportTable {
    let headers = vec![
        "Description".into(),
        "Qty".into(),
        "Unit price".into(),
        "Amount".into(),
    ];
    let rows = inv
        .lines
        .iter()
        .map(|l| {
            vec![
                l.description.clone(),
                l.quantity.to_string(),
                l.unit_price_label.clone(),
                l.amount_label.clone(),
            ]
        })
        .collect();
    crate::routes::reports::ReportTable {
        title: format!("Acre Nexus invoice — {}", period_label(&inv.period)),
        subtitle: Some(format!(
            "{} plan · {} units · {} · due {}",
            saas::plan_for(&inv.plan).name,
            inv.unit_count,
            inv.status,
            inv.due_date.clone().unwrap_or_else(|| "—".into()),
        )),
        headers,
        rows,
        totals: Some(vec![
            "TOTAL".into(),
            String::new(),
            String::new(),
            inv.total_label.clone(),
        ]),
    }
}
