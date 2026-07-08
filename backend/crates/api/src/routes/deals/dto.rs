//! DTOs for the acquisition deal pipeline + underwriting (issues #41/#42).

use crate::deals::stage_label;
use crate::dto::usd;
use crate::underwriting::{underwrite, Assumptions, Underwriting};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One due-diligence checklist item, stored in the deal's `checklist` JSON.
#[derive(Serialize, Deserialize, Clone, Debug, schemars::JsonSchema)]
pub struct ChecklistItemDto {
    pub key: String,
    pub label: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// One point on the rent-growth sensitivity band.
#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct SensitivityDto {
    pub rent_growth_bps: i32,
    pub rent_growth_pct: f64,
    pub irr_bps: Option<i32>,
    pub irr_pct: Option<f64>,
}

/// Computed underwriting for a deal — cap rate, cash-on-cash, DSCR, IRR, and the
/// full operating/sale breakdown, each money value paired with a display label.
#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct UnderwritingDto {
    pub purchase_price_cents: i64,
    pub purchase_price_label: String,
    pub total_project_cost_cents: i64,
    pub total_project_cost_label: String,
    pub loan_amount_cents: i64,
    pub loan_amount_label: String,
    pub down_payment_cents: i64,
    pub down_payment_label: String,
    pub total_cash_invested_cents: i64,
    pub total_cash_invested_label: String,
    pub monthly_debt_service_cents: i64,
    pub monthly_debt_service_label: String,
    pub annual_debt_service_cents: i64,
    pub annual_debt_service_label: String,
    pub gross_rent_annual_cents: i64,
    pub gross_rent_annual_label: String,
    pub vacancy_loss_cents: i64,
    pub vacancy_loss_label: String,
    pub effective_gross_income_cents: i64,
    pub effective_gross_income_label: String,
    pub operating_expenses_annual_cents: i64,
    pub operating_expenses_annual_label: String,
    pub noi_annual_cents: i64,
    pub noi_annual_label: String,
    pub annual_cash_flow_cents: i64,
    pub annual_cash_flow_label: String,
    pub cap_rate_bps: i32,
    pub cap_rate_pct: f64,
    pub cash_on_cash_bps: i32,
    pub cash_on_cash_pct: f64,
    pub dscr: f64,
    pub exit_value_cents: i64,
    pub exit_value_label: String,
    pub loan_balance_at_exit_cents: i64,
    pub loan_balance_at_exit_label: String,
    pub net_sale_proceeds_cents: i64,
    pub net_sale_proceeds_label: String,
    pub irr_bps: Option<i32>,
    pub irr_pct: Option<f64>,
    pub total_profit_cents: i64,
    pub total_profit_label: String,
    pub sensitivity: Vec<SensitivityDto>,
}

impl From<Underwriting> for UnderwritingDto {
    fn from(u: Underwriting) -> Self {
        UnderwritingDto {
            purchase_price_cents: u.purchase_price_cents,
            purchase_price_label: usd(u.purchase_price_cents),
            total_project_cost_cents: u.total_project_cost_cents,
            total_project_cost_label: usd(u.total_project_cost_cents),
            loan_amount_cents: u.loan_amount_cents,
            loan_amount_label: usd(u.loan_amount_cents),
            down_payment_cents: u.down_payment_cents,
            down_payment_label: usd(u.down_payment_cents),
            total_cash_invested_cents: u.total_cash_invested_cents,
            total_cash_invested_label: usd(u.total_cash_invested_cents),
            monthly_debt_service_cents: u.monthly_debt_service_cents,
            monthly_debt_service_label: usd(u.monthly_debt_service_cents),
            annual_debt_service_cents: u.annual_debt_service_cents,
            annual_debt_service_label: usd(u.annual_debt_service_cents),
            gross_rent_annual_cents: u.gross_rent_annual_cents,
            gross_rent_annual_label: usd(u.gross_rent_annual_cents),
            vacancy_loss_cents: u.vacancy_loss_cents,
            vacancy_loss_label: usd(u.vacancy_loss_cents),
            effective_gross_income_cents: u.effective_gross_income_cents,
            effective_gross_income_label: usd(u.effective_gross_income_cents),
            operating_expenses_annual_cents: u.operating_expenses_annual_cents,
            operating_expenses_annual_label: usd(u.operating_expenses_annual_cents),
            noi_annual_cents: u.noi_annual_cents,
            noi_annual_label: usd(u.noi_annual_cents),
            annual_cash_flow_cents: u.annual_cash_flow_cents,
            annual_cash_flow_label: usd(u.annual_cash_flow_cents),
            cap_rate_bps: u.cap_rate_bps,
            cap_rate_pct: u.cap_rate_bps as f64 / 100.0,
            cash_on_cash_bps: u.cash_on_cash_bps,
            cash_on_cash_pct: u.cash_on_cash_bps as f64 / 100.0,
            dscr: (u.dscr * 100.0).round() / 100.0,
            exit_value_cents: u.exit_value_cents,
            exit_value_label: usd(u.exit_value_cents),
            loan_balance_at_exit_cents: u.loan_balance_at_exit_cents,
            loan_balance_at_exit_label: usd(u.loan_balance_at_exit_cents),
            net_sale_proceeds_cents: u.net_sale_proceeds_cents,
            net_sale_proceeds_label: usd(u.net_sale_proceeds_cents),
            irr_bps: u.irr_bps,
            irr_pct: u.irr_bps.map(|b| b as f64 / 100.0),
            total_profit_cents: u.total_profit_cents,
            total_profit_label: usd(u.total_profit_cents),
            sensitivity: u
                .sensitivity
                .into_iter()
                .map(|s| SensitivityDto {
                    rent_growth_bps: s.rent_growth_bps,
                    rent_growth_pct: s.rent_growth_bps as f64 / 100.0,
                    irr_bps: s.irr_bps,
                    irr_pct: s.irr_bps.map(|b| b as f64 / 100.0),
                })
                .collect(),
        }
    }
}

/// A deal plus its computed underwriting and parsed checklist. Money fields
/// carry a `*_label`; assumption knobs are raw `*_bps` / years for the form.
#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct DealDto {
    pub id: Uuid,
    pub name: String,
    pub address: String,
    pub city: String,
    pub stage: String,
    pub stage_label: String,
    pub strategy: String,
    pub property_type: Option<String>,
    pub source: Option<String>,
    pub broker_id: Option<Uuid>,
    pub notes: Option<String>,

    pub asking_price_cents: Option<i64>,
    pub asking_price_label: Option<String>,
    pub offer_price_cents: Option<i64>,
    pub offer_price_label: Option<String>,
    pub earnest_money_cents: Option<i64>,
    pub earnest_money_label: Option<String>,
    pub target_close_on: Option<String>,

    pub arv_cents: Option<i64>,
    pub arv_label: Option<String>,
    pub rehab_budget_cents: Option<i64>,
    pub rehab_budget_label: Option<String>,
    pub closing_costs_cents: Option<i64>,
    pub est_monthly_rent_cents: Option<i64>,
    pub est_monthly_rent_label: Option<String>,
    pub est_monthly_expenses_cents: Option<i64>,
    pub vacancy_bps: Option<i32>,
    pub down_payment_bps: Option<i32>,
    pub interest_rate_bps: Option<i32>,
    pub loan_term_years: Option<i32>,
    pub rent_growth_bps: Option<i32>,
    pub appreciation_bps: Option<i32>,
    pub exit_cap_rate_bps: Option<i32>,
    pub selling_costs_bps: Option<i32>,
    pub hold_years: Option<i32>,

    pub checklist: Vec<ChecklistItemDto>,
    pub converted_property_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
    /// Underwriting computed from the deal's stored assumptions.
    pub underwriting: UnderwritingDto,
}

/// Parse the deal's `checklist` JSON column into typed items (tolerating a
/// malformed/empty value).
pub fn parse_checklist(raw: &serde_json::Value) -> Vec<ChecklistItemDto> {
    serde_json::from_value::<Vec<ChecklistItemDto>>(raw.clone()).unwrap_or_default()
}

impl DealDto {
    /// Build the DTO for a deal, computing underwriting from its stored
    /// assumptions.
    pub fn build(d: &entity::deal::Model) -> Self {
        let assumptions = resolve_assumptions(d, None);
        let underwriting = UnderwritingDto::from(underwrite(&assumptions));
        let money = |c: Option<i64>| c.map(usd);
        DealDto {
            id: d.id,
            name: d.name.clone(),
            address: d.address.clone(),
            city: d.city.clone(),
            stage: d.stage.clone(),
            stage_label: stage_label(&d.stage).to_string(),
            strategy: d.strategy.clone(),
            property_type: d.property_type.clone(),
            source: d.source.clone(),
            broker_id: d.broker_id,
            notes: d.notes.clone(),
            asking_price_cents: d.asking_price_cents,
            asking_price_label: money(d.asking_price_cents),
            offer_price_cents: d.offer_price_cents,
            offer_price_label: money(d.offer_price_cents),
            earnest_money_cents: d.earnest_money_cents,
            earnest_money_label: money(d.earnest_money_cents),
            target_close_on: d.target_close_on.clone(),
            arv_cents: d.arv_cents,
            arv_label: money(d.arv_cents),
            rehab_budget_cents: d.rehab_budget_cents,
            rehab_budget_label: money(d.rehab_budget_cents),
            closing_costs_cents: d.closing_costs_cents,
            est_monthly_rent_cents: d.est_monthly_rent_cents,
            est_monthly_rent_label: money(d.est_monthly_rent_cents),
            est_monthly_expenses_cents: d.est_monthly_expenses_cents,
            vacancy_bps: d.vacancy_bps,
            down_payment_bps: d.down_payment_bps,
            interest_rate_bps: d.interest_rate_bps,
            loan_term_years: d.loan_term_years,
            rent_growth_bps: d.rent_growth_bps,
            appreciation_bps: d.appreciation_bps,
            exit_cap_rate_bps: d.exit_cap_rate_bps,
            selling_costs_bps: d.selling_costs_bps,
            hold_years: d.hold_years,
            checklist: parse_checklist(&d.checklist),
            converted_property_id: d.converted_property_id,
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
            underwriting,
        }
    }
}

/// A deal with its event timeline.
#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct DealDetailDto {
    #[serde(flatten)]
    pub deal: DealDto,
    pub events: Vec<DealEventDto>,
}

#[derive(Serialize, Clone, Debug, schemars::JsonSchema)]
pub struct DealEventDto {
    pub id: Uuid,
    pub kind: String,
    pub from_stage: Option<String>,
    pub to_stage: Option<String>,
    pub body: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub created_at: String,
}

impl From<entity::deal_event::Model> for DealEventDto {
    fn from(e: entity::deal_event::Model) -> Self {
        DealEventDto {
            id: e.id,
            kind: e.kind,
            from_stage: e.from_stage,
            to_stage: e.to_stage,
            body: e.body,
            actor_user_id: e.actor_user_id,
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

/// Create-deal request. Only `name` is required; everything else is optional and
/// can be filled in as the deal is worked.
#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateDealReq {
    pub name: String,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(default)]
    pub property_type: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub broker_id: Option<Uuid>,
    #[serde(default)]
    pub asking_price_cents: Option<i64>,
    #[serde(default)]
    pub offer_price_cents: Option<i64>,
    #[serde(default)]
    pub est_monthly_rent_cents: Option<i64>,
    #[serde(default)]
    pub rehab_budget_cents: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// PATCH request — every field optional; assumption knobs update the stored
/// underwriting inputs. A field set to its JSON `null` is treated as "no change"
/// (there is no field-clearing verb; empty strings clear text fields).
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct UpdateDealReq {
    pub name: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub strategy: Option<String>,
    pub property_type: Option<String>,
    pub source: Option<String>,
    pub broker_id: Option<Uuid>,
    pub notes: Option<String>,
    pub asking_price_cents: Option<i64>,
    pub offer_price_cents: Option<i64>,
    pub earnest_money_cents: Option<i64>,
    pub target_close_on: Option<String>,
    pub arv_cents: Option<i64>,
    pub rehab_budget_cents: Option<i64>,
    pub closing_costs_cents: Option<i64>,
    pub est_monthly_rent_cents: Option<i64>,
    pub est_monthly_expenses_cents: Option<i64>,
    pub vacancy_bps: Option<i32>,
    pub down_payment_bps: Option<i32>,
    pub interest_rate_bps: Option<i32>,
    pub loan_term_years: Option<i32>,
    pub rent_growth_bps: Option<i32>,
    pub appreciation_bps: Option<i32>,
    pub exit_cap_rate_bps: Option<i32>,
    pub selling_costs_bps: Option<i32>,
    pub hold_years: Option<i32>,
}

/// Stateless underwrite ("what-if") request — any subset of assumption knobs
/// overrides the deal's stored values for this computation only.
#[derive(Deserialize, schemars::JsonSchema, Default)]
pub struct UnderwriteReq {
    pub purchase_price_cents: Option<i64>,
    pub rehab_budget_cents: Option<i64>,
    pub closing_costs_cents: Option<i64>,
    pub arv_cents: Option<i64>,
    pub est_monthly_rent_cents: Option<i64>,
    pub est_monthly_expenses_cents: Option<i64>,
    pub vacancy_bps: Option<i32>,
    pub down_payment_bps: Option<i32>,
    pub interest_rate_bps: Option<i32>,
    pub loan_term_years: Option<i32>,
    pub rent_growth_bps: Option<i32>,
    pub appreciation_bps: Option<i32>,
    pub exit_cap_rate_bps: Option<i32>,
    pub selling_costs_bps: Option<i32>,
    pub hold_years: Option<i32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AdvanceStageReq {
    pub stage: String,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateChecklistReq {
    pub checklist: Vec<ChecklistItemDto>,
}

/// Result of converting a deal into an owned property.
#[derive(Serialize, schemars::JsonSchema)]
pub struct ConvertResp {
    pub deal: DealDto,
    pub property_id: Uuid,
}

/// Resolve the effective [`Assumptions`] for a deal: an override (what-if) wins,
/// then the deal's stored value, then the engine default. Purchase price falls
/// through offer → asking.
pub fn resolve_assumptions(d: &entity::deal::Model, o: Option<&UnderwriteReq>) -> Assumptions {
    let def = Assumptions::default();
    Assumptions {
        purchase_price_cents: o
            .and_then(|x| x.purchase_price_cents)
            .or(d.offer_price_cents)
            .or(d.asking_price_cents)
            .unwrap_or(0),
        rehab_cents: o
            .and_then(|x| x.rehab_budget_cents)
            .or(d.rehab_budget_cents)
            .unwrap_or(0),
        closing_costs_cents: o
            .and_then(|x| x.closing_costs_cents)
            .or(d.closing_costs_cents)
            .unwrap_or(0),
        arv_cents: o.and_then(|x| x.arv_cents).or(d.arv_cents).unwrap_or(0),
        monthly_rent_cents: o
            .and_then(|x| x.est_monthly_rent_cents)
            .or(d.est_monthly_rent_cents)
            .unwrap_or(0),
        monthly_expenses_cents: o
            .and_then(|x| x.est_monthly_expenses_cents)
            .or(d.est_monthly_expenses_cents)
            .unwrap_or(0),
        vacancy_bps: o
            .and_then(|x| x.vacancy_bps)
            .or(d.vacancy_bps)
            .unwrap_or(def.vacancy_bps),
        down_payment_bps: o
            .and_then(|x| x.down_payment_bps)
            .or(d.down_payment_bps)
            .unwrap_or(def.down_payment_bps),
        interest_rate_bps: o
            .and_then(|x| x.interest_rate_bps)
            .or(d.interest_rate_bps)
            .unwrap_or(def.interest_rate_bps),
        loan_term_years: o
            .and_then(|x| x.loan_term_years)
            .or(d.loan_term_years)
            .unwrap_or(def.loan_term_years),
        rent_growth_bps: o
            .and_then(|x| x.rent_growth_bps)
            .or(d.rent_growth_bps)
            .unwrap_or(def.rent_growth_bps),
        appreciation_bps: o
            .and_then(|x| x.appreciation_bps)
            .or(d.appreciation_bps)
            .unwrap_or(def.appreciation_bps),
        exit_cap_rate_bps: o
            .and_then(|x| x.exit_cap_rate_bps)
            .or(d.exit_cap_rate_bps)
            .unwrap_or(def.exit_cap_rate_bps),
        selling_costs_bps: o
            .and_then(|x| x.selling_costs_bps)
            .or(d.selling_costs_bps)
            .unwrap_or(def.selling_costs_bps),
        hold_years: o
            .and_then(|x| x.hold_years)
            .or(d.hold_years)
            .unwrap_or(def.hold_years),
    }
}
