"use client";

// Acquisition deal detail — the investor underwriting workspace for one deal:
// an interactive cap-rate / cash-on-cash / IRR / DSCR calculator with a
// rent-growth sensitivity band, offer terms, a due-diligence checklist, the
// deal data room (document service), a stage tracker, an event timeline, and
// one-click conversion into an owned property.

import { useCallback, useEffect, useMemo, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import Link from "next/link";
import {
  api,
  type DealDetail,
  type DealUnderwriting,
  type DealChecklistItem,
  type UnderwriteInput,
  type UpdateDealInput,
} from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card, StatTile } from "@/components/ui";
import { DocumentsCard } from "@/components/DocumentsCard";
import { logError } from "@/lib/log";

const STAGES = [
  { key: "prospecting", label: "Prospecting" },
  { key: "offer", label: "Offer" },
  { key: "under_contract", label: "Under contract" },
  { key: "closing", label: "Closing" },
  { key: "owned", label: "Owned" },
];

const DEFAULT_CHECKLIST: DealChecklistItem[] = [
  { key: "inspection", label: "General inspection", done: false },
  { key: "title", label: "Title search / commitment", done: false },
  { key: "appraisal", label: "Appraisal / valuation", done: false },
  { key: "bids", label: "Contractor rehab bids", done: false },
  { key: "financing", label: "Financing commitment", done: false },
  { key: "insurance", label: "Insurance quote", done: false },
];

/** Dollars string → integer cents, or undefined if blank/NaN. */
function dollarsToCents(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  return Number.isNaN(n) ? undefined : Math.round(n * 100);
}
/** Percent string (e.g. "6.5") → integer basis points, or undefined. */
function percentToBps(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  return Number.isNaN(n) ? undefined : Math.round(n * 100);
}
function toInt(v: string): number | undefined {
  const t = v.trim();
  if (t === "") return undefined;
  const n = Number(t);
  return Number.isNaN(n) ? undefined : Math.round(n);
}
const centsToDollars = (c: number | null | undefined) =>
  c === null || c === undefined ? "" : (c / 100).toString();
const bpsToPercent = (b: number | null | undefined) =>
  b === null || b === undefined ? "" : (b / 100).toString();
const fmtPct = (v: number | null) => (v === null ? "—" : `${v.toFixed(1)}%`);

type Form = Record<string, string>;

function formFromDeal(d: DealDetail): Form {
  return {
    asking_price: centsToDollars(d.asking_price_cents),
    offer_price: centsToDollars(d.offer_price_cents),
    earnest_money: centsToDollars(d.earnest_money_cents),
    target_close_on: d.target_close_on ?? "",
    arv: centsToDollars(d.arv_cents),
    rehab_budget: centsToDollars(d.rehab_budget_cents),
    closing_costs: centsToDollars(d.closing_costs_cents),
    est_monthly_rent: centsToDollars(d.est_monthly_rent_cents),
    est_monthly_expenses: centsToDollars(d.est_monthly_expenses_cents),
    down_payment: bpsToPercent(d.down_payment_bps ?? 2000),
    interest_rate: bpsToPercent(d.interest_rate_bps ?? 700),
    vacancy: bpsToPercent(d.vacancy_bps ?? 500),
    rent_growth: bpsToPercent(d.rent_growth_bps ?? 300),
    appreciation: bpsToPercent(d.appreciation_bps ?? 300),
    exit_cap_rate: bpsToPercent(d.exit_cap_rate_bps),
    selling_costs: bpsToPercent(d.selling_costs_bps ?? 700),
    loan_term_years: (d.loan_term_years ?? 30).toString(),
    hold_years: (d.hold_years ?? 5).toString(),
    notes: d.notes ?? "",
  };
}

function underwriteInputFrom(f: Form): UnderwriteInput {
  const offer = dollarsToCents(f.offer_price);
  const asking = dollarsToCents(f.asking_price);
  return {
    purchase_price_cents: offer ?? asking,
    arv_cents: dollarsToCents(f.arv),
    rehab_budget_cents: dollarsToCents(f.rehab_budget),
    closing_costs_cents: dollarsToCents(f.closing_costs),
    est_monthly_rent_cents: dollarsToCents(f.est_monthly_rent),
    est_monthly_expenses_cents: dollarsToCents(f.est_monthly_expenses),
    vacancy_bps: percentToBps(f.vacancy),
    down_payment_bps: percentToBps(f.down_payment),
    interest_rate_bps: percentToBps(f.interest_rate),
    loan_term_years: toInt(f.loan_term_years),
    rent_growth_bps: percentToBps(f.rent_growth),
    appreciation_bps: percentToBps(f.appreciation),
    exit_cap_rate_bps: percentToBps(f.exit_cap_rate),
    selling_costs_bps: percentToBps(f.selling_costs),
    hold_years: toInt(f.hold_years),
  };
}

function updateInputFrom(f: Form): UpdateDealInput {
  return {
    asking_price_cents: dollarsToCents(f.asking_price),
    offer_price_cents: dollarsToCents(f.offer_price),
    earnest_money_cents: dollarsToCents(f.earnest_money),
    target_close_on: f.target_close_on.trim() || undefined,
    arv_cents: dollarsToCents(f.arv),
    rehab_budget_cents: dollarsToCents(f.rehab_budget),
    closing_costs_cents: dollarsToCents(f.closing_costs),
    est_monthly_rent_cents: dollarsToCents(f.est_monthly_rent),
    est_monthly_expenses_cents: dollarsToCents(f.est_monthly_expenses),
    vacancy_bps: percentToBps(f.vacancy),
    down_payment_bps: percentToBps(f.down_payment),
    interest_rate_bps: percentToBps(f.interest_rate),
    loan_term_years: toInt(f.loan_term_years),
    rent_growth_bps: percentToBps(f.rent_growth),
    appreciation_bps: percentToBps(f.appreciation),
    exit_cap_rate_bps: percentToBps(f.exit_cap_rate),
    selling_costs_bps: percentToBps(f.selling_costs),
    hold_years: toInt(f.hold_years),
    notes: f.notes,
  };
}

function Field({
  label,
  suffix,
  value,
  onChange,
  placeholder,
  type = "text",
}: {
  label: string;
  suffix?: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  type?: string;
}) {
  return (
    <label className="text-sm">
      <span className="mb-1 block text-xs font-semibold text-ink-3">
        {label}
        {suffix ? ` (${suffix})` : ""}
      </span>
      <input
        type={type}
        value={value}
        placeholder={placeholder}
        inputMode={suffix === "$" || suffix === "%" ? "decimal" : undefined}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-lg border border-line bg-surface-2 px-3 py-2"
      />
    </label>
  );
}

function MetricRow({ u }: { u: DealUnderwriting }) {
  return (
    <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
      <StatTile label="Cap rate" value={fmtPct(u.cap_rate_pct)} icon="chart" />
      <StatTile
        label="Cash-on-cash"
        value={fmtPct(u.cash_on_cash_pct)}
        icon="dollar"
      />
      <StatTile label="IRR" value={fmtPct(u.irr_pct)} icon="chart" />
      <StatTile label="DSCR" value={`${u.dscr.toFixed(2)}×`} icon="bank" />
      <StatTile
        label="Cash flow / yr"
        value={u.annual_cash_flow_label}
        icon="dollar"
      />
    </div>
  );
}

function Breakdown({ u }: { u: DealUnderwriting }) {
  const rows: [string, string][] = [
    ["All-in cost basis", u.total_project_cost_label],
    ["Loan amount", u.loan_amount_label],
    ["Cash invested", u.total_cash_invested_label],
    ["Gross rent / yr", u.gross_rent_annual_label],
    ["Vacancy loss", `-${u.vacancy_loss_label}`],
    ["Operating expenses", `-${u.operating_expenses_annual_label}`],
    ["Net operating income", u.noi_annual_label],
    ["Annual debt service", `-${u.annual_debt_service_label}`],
    ["Projected exit value", u.exit_value_label],
    ["Loan payoff at exit", `-${u.loan_balance_at_exit_label}`],
    ["Net sale proceeds", u.net_sale_proceeds_label],
    ["Total profit (hold)", u.total_profit_label],
  ];
  return (
    <div className="grid gap-x-6 gap-y-1 sm:grid-cols-2">
      {rows.map(([k, v]) => (
        <div
          key={k}
          className="flex items-center justify-between border-b border-line py-1.5 text-sm"
        >
          <span className="text-ink-3">{k}</span>
          <span className="font-semibold tabular-nums">{v}</span>
        </div>
      ))}
    </div>
  );
}

function Sensitivity({ u }: { u: DealUnderwriting }) {
  const irrs = u.sensitivity
    .map((s) => s.irr_bps)
    .filter((b): b is number => b !== null);
  const max = irrs.length ? Math.max(...irrs, 1) : 1;
  return (
    <div className="space-y-2">
      <div className="text-xs text-ink-3">
        IRR as annual rent growth varies ±2 points:
      </div>
      {u.sensitivity.map((s) => {
        const w = s.irr_bps === null ? 0 : Math.max(0, (s.irr_bps / max) * 100);
        return (
          <div key={s.rent_growth_bps} className="flex items-center gap-3">
            <span className="w-24 shrink-0 text-xs text-ink-3">
              growth {fmtPct(s.rent_growth_pct)}
            </span>
            <div className="h-3 flex-1 overflow-hidden rounded-full bg-surface-2">
              <div
                className="h-full rounded-full bg-accent"
                style={{ width: `${w}%` }}
              />
            </div>
            <span className="w-14 shrink-0 text-right text-xs font-semibold tabular-nums">
              {fmtPct(s.irr_pct)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

export default function DealDetailPage() {
  const params = useParams<{ id: string }>();
  const id = params?.id;
  const router = useRouter();
  const { can } = useAuth();
  const canWrite = can("deal:write");

  const [deal, setDeal] = useState<DealDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState<Form>({});
  const [preview, setPreview] = useState<DealUnderwriting | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => {
    if (!id) return;
    api
      .flipDeal(id)
      .then((d) => {
        setDeal(d);
        setForm(formFromDeal(d));
        setPreview(null);
      })
      .catch((e) => setError(e.message));
  }, [id]);

  useEffect(() => {
    load();
  }, [load]);

  const setField = (k: string) => (v: string) =>
    setForm((f) => ({ ...f, [k]: v }));

  const shown = useMemo(
    () => preview ?? deal?.underwriting ?? null,
    [preview, deal]
  );

  async function recalc() {
    if (!id) return;
    setBusy("recalc");
    try {
      const u = await api.underwriteFlipDeal(id, underwriteInputFrom(form));
      setPreview(u);
    } catch (e) {
      logError("failed to underwrite", e);
    } finally {
      setBusy(null);
    }
  }

  async function save() {
    if (!id) return;
    setBusy("save");
    try {
      await api.updateFlipDeal(id, updateInputFrom(form));
      load();
    } catch (e) {
      logError("failed to save deal", e);
    } finally {
      setBusy(null);
    }
  }

  async function advance(stage: string) {
    if (!id) return;
    try {
      await api.advanceFlipDealStage(id, stage);
      load();
    } catch (e) {
      logError("failed to advance stage", e);
    }
  }

  async function toggleChecklist(item: DealChecklistItem) {
    if (!id || !deal) return;
    const next = deal.checklist.map((c) =>
      c.key === item.key ? { ...c, done: !c.done } : c
    );
    try {
      const updated = await api.updateFlipChecklist(id, next);
      setDeal({ ...deal, checklist: updated.checklist });
    } catch (e) {
      logError("failed to update checklist", e);
    }
  }

  async function seedChecklist() {
    if (!id || !deal) return;
    try {
      const updated = await api.updateFlipChecklist(id, DEFAULT_CHECKLIST);
      setDeal({ ...deal, checklist: updated.checklist });
    } catch (e) {
      logError("failed to seed checklist", e);
    }
  }

  async function convert() {
    if (!id) return;
    if (!window.confirm("Convert this deal into an owned property?")) return;
    setBusy("convert");
    try {
      const res = await api.convertFlipDeal(id);
      router.push(`/console/properties/${res.property_id}`);
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Conversion failed.";
      setError(msg);
      logError("failed to convert deal", e);
      setBusy(null);
    }
  }

  if (error) {
    return (
      <div className="rounded-xl border border-bad-soft bg-bad-soft/40 px-4 py-3 text-sm text-bad">
        {error}
      </div>
    );
  }
  if (!deal || !shown) return <div className="text-ink-3">Loading deal…</div>;

  return (
    <div className="space-y-6">
      <div>
        <Link
          href="/console/flips"
          className="text-sm text-ink-3 hover:text-ink"
        >
          ← Acquisitions
        </Link>
      </div>

      <header className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="flex items-center gap-3">
            <h1 className="font-display text-2xl font-bold">{deal.name}</h1>
            <Badge tone="neutral">{deal.strategy}</Badge>
            <Badge tone={deal.stage === "owned" ? "good" : "info"}>
              {deal.stage_label}
            </Badge>
          </div>
          <p className="mt-1 text-sm text-ink-3">
            {[deal.address, deal.city].filter(Boolean).join(", ") || "—"}
          </p>
        </div>
        {deal.converted_property_id ? (
          <Link href={`/console/properties/${deal.converted_property_id}`}>
            <Button variant="outline">View property →</Button>
          </Link>
        ) : (
          canWrite && (
            <Button onClick={convert} disabled={busy === "convert"}>
              {busy === "convert" ? "Converting…" : "Convert to property"}
            </Button>
          )
        )}
      </header>

      {/* Stage tracker */}
      <Card className="p-4">
        <div className="flex flex-wrap gap-2">
          {STAGES.map((s) => {
            const active = s.key === deal.stage;
            return (
              <button
                key={s.key}
                disabled={!canWrite || active}
                onClick={() => advance(s.key)}
                className={
                  active
                    ? "rounded-full bg-accent px-3 py-1.5 text-xs font-bold text-on-accent"
                    : "rounded-full border border-line px-3 py-1.5 text-xs font-semibold text-ink-2 enabled:hover:bg-surface-2 disabled:opacity-50"
                }
              >
                {s.label}
              </button>
            );
          })}
          {canWrite && deal.stage !== "dead" && (
            <button
              onClick={() => advance("dead")}
              className="ml-auto rounded-full border border-line px-3 py-1.5 text-xs font-semibold text-ink-3 hover:bg-surface-2"
            >
              Mark dead
            </button>
          )}
        </div>
      </Card>

      {/* Underwriting */}
      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="font-display text-lg font-bold">Underwriting</h2>
          {preview && (
            <Badge tone="warn">Preview — recalculated, not saved</Badge>
          )}
        </div>
        <MetricRow u={shown} />

        <div className="grid gap-4 lg:grid-cols-2">
          <Card className="space-y-3 p-4">
            <h3 className="font-display text-sm font-bold">Assumptions</h3>
            <div className="grid grid-cols-2 gap-3">
              <Field
                label="Asking price"
                suffix="$"
                value={form.asking_price ?? ""}
                onChange={setField("asking_price")}
              />
              <Field
                label="Offer price"
                suffix="$"
                value={form.offer_price ?? ""}
                onChange={setField("offer_price")}
              />
              <Field
                label="After-repair value"
                suffix="$"
                value={form.arv ?? ""}
                onChange={setField("arv")}
              />
              <Field
                label="Rehab budget"
                suffix="$"
                value={form.rehab_budget ?? ""}
                onChange={setField("rehab_budget")}
              />
              <Field
                label="Closing costs"
                suffix="$"
                value={form.closing_costs ?? ""}
                onChange={setField("closing_costs")}
              />
              <Field
                label="Earnest money"
                suffix="$"
                value={form.earnest_money ?? ""}
                onChange={setField("earnest_money")}
              />
              <Field
                label="Monthly rent"
                suffix="$"
                value={form.est_monthly_rent ?? ""}
                onChange={setField("est_monthly_rent")}
              />
              <Field
                label="Monthly expenses"
                suffix="$"
                value={form.est_monthly_expenses ?? ""}
                onChange={setField("est_monthly_expenses")}
              />
              <Field
                label="Down payment"
                suffix="%"
                value={form.down_payment ?? ""}
                onChange={setField("down_payment")}
              />
              <Field
                label="Interest rate"
                suffix="%"
                value={form.interest_rate ?? ""}
                onChange={setField("interest_rate")}
              />
              <Field
                label="Loan term"
                suffix="yrs"
                value={form.loan_term_years ?? ""}
                onChange={setField("loan_term_years")}
              />
              <Field
                label="Vacancy"
                suffix="%"
                value={form.vacancy ?? ""}
                onChange={setField("vacancy")}
              />
              <Field
                label="Rent growth"
                suffix="%"
                value={form.rent_growth ?? ""}
                onChange={setField("rent_growth")}
              />
              <Field
                label="Appreciation"
                suffix="%"
                value={form.appreciation ?? ""}
                onChange={setField("appreciation")}
              />
              <Field
                label="Exit cap rate"
                suffix="%"
                value={form.exit_cap_rate ?? ""}
                onChange={setField("exit_cap_rate")}
                placeholder="blank = use appreciation"
              />
              <Field
                label="Selling costs"
                suffix="%"
                value={form.selling_costs ?? ""}
                onChange={setField("selling_costs")}
              />
              <Field
                label="Hold"
                suffix="yrs"
                value={form.hold_years ?? ""}
                onChange={setField("hold_years")}
              />
              <Field
                label="Target close"
                type="date"
                value={form.target_close_on ?? ""}
                onChange={setField("target_close_on")}
              />
            </div>
            {canWrite && (
              <div className="flex gap-2 pt-1">
                <Button
                  variant="outline"
                  onClick={recalc}
                  disabled={busy === "recalc"}
                >
                  {busy === "recalc" ? "Calculating…" : "Recalculate"}
                </Button>
                <Button onClick={save} disabled={busy === "save"}>
                  {busy === "save" ? "Saving…" : "Save assumptions"}
                </Button>
              </div>
            )}
          </Card>

          <div className="space-y-4">
            <Card className="p-4">
              <h3 className="mb-3 font-display text-sm font-bold">
                Returns breakdown
              </h3>
              <Breakdown u={shown} />
            </Card>
            <Card className="p-4">
              <h3 className="mb-3 font-display text-sm font-bold">
                Sensitivity
              </h3>
              <Sensitivity u={shown} />
            </Card>
          </div>
        </div>
      </section>

      {/* Due-diligence + data room */}
      <section className="grid gap-4 lg:grid-cols-2">
        <Card className="p-4">
          <h3 className="mb-3 font-display text-sm font-bold">
            Due-diligence checklist
          </h3>
          {deal.checklist.length === 0 ? (
            <div className="space-y-2 text-sm text-ink-3">
              <p>No checklist yet.</p>
              {canWrite && (
                <Button variant="outline" onClick={seedChecklist}>
                  Add standard checklist
                </Button>
              )}
            </div>
          ) : (
            <ul className="space-y-2">
              {deal.checklist.map((c) => (
                <li key={c.key} className="flex items-center gap-3 text-sm">
                  <input
                    type="checkbox"
                    checked={c.done}
                    disabled={!canWrite}
                    onChange={() => toggleChecklist(c)}
                    className="h-4 w-4 accent-[var(--accent)]"
                  />
                  <span
                    className={c.done ? "text-ink-3 line-through" : "text-ink"}
                  >
                    {c.label}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Card>

        <DocumentsCard ownerType="deal" ownerId={deal.id} title="Data room" />
      </section>

      {/* Timeline */}
      <section>
        <h3 className="mb-3 font-display text-sm font-bold">Timeline</h3>
        <Card className="divide-y divide-line">
          {deal.events.length === 0 ? (
            <div className="p-4 text-sm text-ink-3">No activity yet.</div>
          ) : (
            deal.events.map((e) => (
              <div
                key={e.id}
                className="flex items-start justify-between gap-3 p-3 text-sm"
              >
                <div>
                  <span className="font-semibold">
                    {e.kind === "stage_change"
                      ? `Moved ${e.from_stage ?? "?"} → ${e.to_stage ?? "?"}`
                      : e.kind === "created"
                        ? "Deal created"
                        : e.kind === "converted"
                          ? "Converted to property"
                          : e.kind}
                  </span>
                  {e.body && <p className="text-ink-3">{e.body}</p>}
                </div>
                <span className="shrink-0 text-xs text-ink-3">
                  {new Date(e.created_at).toLocaleDateString()}
                </span>
              </div>
            ))
          )}
        </Card>
      </section>
    </div>
  );
}
