// Shared bits for the acquisition pages: strategies, stages, the default
// due-diligence list and the form conversions for the underwriting inputs.

import type {
  DealChecklistItem,
  DealDetail,
  UnderwriteInput,
  UpdateDealInput,
} from "@/lib/api";

export const STRATEGIES = [
  { key: "flip", label: "Fix and flip" },
  { key: "brrrr", label: "BRRRR" },
  { key: "rental", label: "Buy and hold" },
  { key: "hold", label: "Land or hold" },
  { key: "wholesale", label: "Wholesale" },
];

export const strategyLabel = (k: string) =>
  STRATEGIES.find((s) => s.key === k)?.label ?? k;

export const STAGES = [
  { key: "prospecting", label: "Prospecting" },
  { key: "offer", label: "Offer" },
  { key: "under_contract", label: "Under contract" },
  { key: "closing", label: "Closing" },
  { key: "owned", label: "Owned" },
];

export const DEFAULT_CHECKLIST: DealChecklistItem[] = [
  { key: "inspection", label: "General inspection", done: false },
  { key: "title", label: "Title search and commitment", done: false },
  { key: "appraisal", label: "Appraisal or valuation", done: false },
  { key: "bids", label: "Contractor rehab bids", done: false },
  { key: "financing", label: "Financing commitment", done: false },
  { key: "insurance", label: "Insurance quote", done: false },
];

export const pct = (v: number | null) =>
  v === null ? "—" : `${v.toFixed(1)}%`;

function num(v: string): number | undefined {
  const t = v.trim().replace(/[$,%]/g, "");
  if (t === "") return undefined;
  const n = Number(t);
  return Number.isNaN(n) ? undefined : n;
}
/** Dollars → cents. */
export const cents = (v: string) => {
  const n = num(v);
  return n === undefined ? undefined : Math.round(n * 100);
};
/** Percent → basis points. */
export const bps = cents;
const int = (v: string) => {
  const n = num(v);
  return n === undefined ? undefined : Math.round(n);
};
const dollars = (c: number | null | undefined) =>
  c === null || c === undefined ? "" : String(c / 100);
const percent = dollars;

export type DealForm = Record<string, string>;

export function formFromDeal(d: DealDetail): DealForm {
  return {
    asking_price: dollars(d.asking_price_cents),
    offer_price: dollars(d.offer_price_cents),
    earnest_money: dollars(d.earnest_money_cents),
    target_close_on: d.target_close_on ?? "",
    arv: dollars(d.arv_cents),
    rehab_budget: dollars(d.rehab_budget_cents),
    closing_costs: dollars(d.closing_costs_cents),
    est_monthly_rent: dollars(d.est_monthly_rent_cents),
    est_monthly_expenses: dollars(d.est_monthly_expenses_cents),
    down_payment: percent(d.down_payment_bps ?? 2000),
    interest_rate: percent(d.interest_rate_bps ?? 700),
    vacancy: percent(d.vacancy_bps ?? 500),
    rent_growth: percent(d.rent_growth_bps ?? 300),
    appreciation: percent(d.appreciation_bps ?? 300),
    exit_cap_rate: percent(d.exit_cap_rate_bps),
    selling_costs: percent(d.selling_costs_bps ?? 700),
    loan_term_years: String(d.loan_term_years ?? 30),
    hold_years: String(d.hold_years ?? 5),
    notes: d.notes ?? "",
  };
}

function shared(f: DealForm) {
  return {
    arv_cents: cents(f.arv),
    rehab_budget_cents: cents(f.rehab_budget),
    closing_costs_cents: cents(f.closing_costs),
    est_monthly_rent_cents: cents(f.est_monthly_rent),
    est_monthly_expenses_cents: cents(f.est_monthly_expenses),
    vacancy_bps: bps(f.vacancy),
    down_payment_bps: bps(f.down_payment),
    interest_rate_bps: bps(f.interest_rate),
    loan_term_years: int(f.loan_term_years),
    rent_growth_bps: bps(f.rent_growth),
    appreciation_bps: bps(f.appreciation),
    exit_cap_rate_bps: bps(f.exit_cap_rate),
    selling_costs_bps: bps(f.selling_costs),
    hold_years: int(f.hold_years),
  };
}

export function underwriteInput(f: DealForm): UnderwriteInput {
  return {
    purchase_price_cents: cents(f.offer_price) ?? cents(f.asking_price),
    ...shared(f),
  };
}

export function updateInput(f: DealForm): UpdateDealInput {
  return {
    asking_price_cents: cents(f.asking_price),
    offer_price_cents: cents(f.offer_price),
    earnest_money_cents: cents(f.earnest_money),
    target_close_on: f.target_close_on.trim() || undefined,
    ...shared(f),
    notes: f.notes,
  };
}
