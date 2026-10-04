"use client";

// One acquisition: the underwriting calculator (cap rate, cash-on-cash, IRR,
// DSCR) with a rent-growth sensitivity band, offer terms, the due-diligence
// checklist, the data room, the stage tracker, the timeline, and turning a
// closed deal into an owned property.

import { useState } from "react";
import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowLeft,
  ArrowRight,
  Building2,
  Calculator,
  ClipboardCheck,
  History,
  Save,
  Skull,
  TrendingUp,
} from "lucide-react";
import { toast } from "sonner";
import {
  api,
  ApiError,
  type DealDetail,
  type DealUnderwriting,
} from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, input } from "@/components/property/bits";
import { cn } from "@/lib/utils";
import { Documents } from "../_parts/Documents";
import { Land } from "../_parts/Land";
import {
  DEFAULT_CHECKLIST,
  formFromDeal,
  pct,
  STAGES,
  strategyLabel,
  underwriteInput,
  updateInput,
  type DealForm,
} from "../deal";

export default function DealPage() {
  const { id } = useParams<{ id: string }>();
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const router = useRouter();
  const qc = useQueryClient();
  const write = can("deal:write");
  const key = ["flips", "one", id];
  const deal = useQuery({
    queryKey: key,
    queryFn: () => api.flipDeal(id),
    enabled: scoped && can("deal:read"),
  });
  const [converting, setConverting] = useState(false);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: key });
    void qc.invalidateQueries({ queryKey: ["flips"], exact: true });
  };

  async function advance(stage: string) {
    try {
      await api.advanceFlipDealStage(id, stage);
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't move the deal");
    }
  }

  async function convert() {
    if (!confirm("Turn this deal into an owned property?")) return;
    setConverting(true);
    try {
      const res = await api.convertFlipDeal(id);
      toast.success("Property created");
      void qc.invalidateQueries({ queryKey: ["properties"] });
      router.push(`/console/properties/${res.property_id}`);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't convert it");
      setConverting(false);
    }
  }

  const back = (
    <Link
      href="/console/flips"
      className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
    >
      <ArrowLeft className="size-4" />
      Acquisitions
    </Link>
  );

  if (deal.error) {
    const missing = deal.error instanceof ApiError && deal.error.status === 404;
    return (
      <div className="space-y-6">
        {back}
        <Panel className="mx-auto max-w-lg">
          <EmptyState
            icon={<TrendingUp />}
            title={missing ? "This deal isn't here" : "Couldn't load the deal"}
            description={missing ? undefined : deal.error.message}
          />
        </Panel>
      </div>
    );
  }
  const d = deal.data;
  if (!d)
    return (
      <div className="space-y-6">
        {back}
        <Skeleton className="h-20 rounded-2xl" />
        <Skeleton className="h-96 rounded-2xl" />
      </div>
    );

  return (
    <div className="space-y-6">
      {back}
      <PageHeader
        eyebrow={
          <span className="inline-flex items-center gap-2">
            {strategyLabel(d.strategy)}
            <Badge
              tone={
                d.stage === "owned"
                  ? "good"
                  : d.stage === "dead"
                    ? "bad"
                    : "info"
              }
            >
              {d.stage_label}
            </Badge>
          </span>
        }
        title={d.name}
        description={
          [d.address, d.city].filter(Boolean).join(", ") || "No address yet"
        }
        actions={
          d.converted_property_id ? (
            <Button variant="secondary" asChild>
              <Link href={`/console/properties/${d.converted_property_id}`}>
                <Building2 />
                View property
              </Link>
            </Button>
          ) : (
            write &&
            can("property:write") && (
              <Button onClick={convert} loading={converting}>
                <Building2 />
                Convert to property
              </Button>
            )
          )
        }
      />

      <Panel className="flex flex-wrap items-center gap-1.5 p-2">
        {STAGES.map((s, i) => {
          const on = s.key === d.stage;
          return (
            <div key={s.key} className="flex items-center gap-1.5">
              {i > 0 && <ArrowRight className="size-3.5 text-fg-4" />}
              <button
                type="button"
                disabled={!write || on}
                aria-current={on ? "step" : undefined}
                onClick={() => advance(s.key)}
                className={cn(
                  "rounded-lg px-3 py-1.5 text-[13px] font-medium transition",
                  on
                    ? "bg-accent text-accent-fg"
                    : "text-fg-2 enabled:hover:bg-fill-2 enabled:hover:text-fg disabled:opacity-60"
                )}
              >
                {s.label}
              </button>
            </div>
          );
        })}
        {write && d.stage !== "dead" && (
          <Button
            size="sm"
            variant="ghost"
            className="ml-auto text-fg-3"
            onClick={() => {
              if (confirm("Mark this deal dead?")) void advance("dead");
            }}
          >
            <Skull />
            Mark dead
          </Button>
        )}
      </Panel>

      <Underwriting
        key={d.updated_at}
        deal={d}
        write={write}
        onSaved={refresh}
      />

      <Land
        key={`land-${d.updated_at}`}
        deal={d}
        write={write}
        onSaved={refresh}
      />

      <section className="grid gap-4 lg:grid-cols-2">
        <Checklist deal={d} write={write} />
        <Documents
          ownerType="deal"
          ownerId={d.id}
          title="Data room"
          description="Inspection reports, title, bids, the contract."
        />
      </section>

      <Panel>
        <PanelHeader title="Timeline" />
        <div className="p-2 pt-3">
          {d.events.length === 0 ? (
            <EmptyState
              icon={<History />}
              title="No activity yet"
              className="py-8"
            />
          ) : (
            <ul className="divide-y divide-line">
              {d.events.map((e) => (
                <li
                  key={e.id}
                  className="flex items-start justify-between gap-3 px-3 py-2.5"
                >
                  <div className="min-w-0">
                    <div className="text-[13px] font-medium text-fg">
                      {e.kind === "stage_change"
                        ? `Moved from ${stageName(e.from_stage)} to ${stageName(e.to_stage)}`
                        : e.kind === "created"
                          ? "Deal created"
                          : e.kind === "converted"
                            ? "Converted to a property"
                            : e.kind.replace(/_/g, " ")}
                    </div>
                    {e.body && <p className="text-xs text-fg-3">{e.body}</p>}
                  </div>
                  <span className="shrink-0 text-xs text-fg-3">
                    {new Date(e.created_at).toLocaleDateString()}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </Panel>
    </div>
  );
}

function stageName(k: string | null) {
  if (!k) return "?";
  if (k === "dead") return "Dead";
  return STAGES.find((s) => s.key === k)?.label ?? k;
}

const FIELDS: { k: string; label: string; unit?: "$" | "%" | "yrs" }[] = [
  { k: "asking_price", label: "Asking price", unit: "$" },
  { k: "offer_price", label: "Offer price", unit: "$" },
  { k: "arv", label: "After-repair value", unit: "$" },
  { k: "rehab_budget", label: "Rehab budget", unit: "$" },
  { k: "closing_costs", label: "Closing costs", unit: "$" },
  { k: "earnest_money", label: "Earnest money", unit: "$" },
  { k: "est_monthly_rent", label: "Monthly rent", unit: "$" },
  { k: "est_monthly_expenses", label: "Monthly expenses", unit: "$" },
  { k: "down_payment", label: "Down payment", unit: "%" },
  { k: "interest_rate", label: "Interest rate", unit: "%" },
  { k: "loan_term_years", label: "Loan term", unit: "yrs" },
  { k: "vacancy", label: "Vacancy", unit: "%" },
  { k: "rent_growth", label: "Rent growth", unit: "%" },
  { k: "appreciation", label: "Appreciation", unit: "%" },
  { k: "exit_cap_rate", label: "Exit cap rate", unit: "%" },
  { k: "selling_costs", label: "Selling costs", unit: "%" },
  { k: "hold_years", label: "Hold", unit: "yrs" },
];

function Underwriting({
  deal,
  write,
  onSaved,
}: {
  deal: DealDetail;
  write: boolean;
  onSaved: () => void;
}) {
  const [form, setForm] = useState<DealForm>(() => formFromDeal(deal));
  const [preview, setPreview] = useState<DealUnderwriting | null>(null);
  const [busy, setBusy] = useState<"recalc" | "save" | null>(null);
  const u = preview ?? deal.underwriting;
  const set = (k: string) => (v: string) => setForm((f) => ({ ...f, [k]: v }));

  async function recalc() {
    setBusy("recalc");
    try {
      setPreview(await api.underwriteFlipDeal(deal.id, underwriteInput(form)));
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't recalculate");
    } finally {
      setBusy(null);
    }
  }

  async function save() {
    setBusy("save");
    try {
      await api.updateFlipDeal(deal.id, updateInput(form));
      toast.success("Assumptions saved");
      onSaved();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save");
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-[17px] font-semibold text-fg">Underwriting</h2>
        {preview && <Badge tone="warn">Recalculated, not saved</Badge>}
      </div>
      <div className="grid grid-cols-2 gap-3 md:grid-cols-5">
        <Stat label="Cap rate" value={pct(u.cap_rate_pct)} />
        <Stat label="Cash-on-cash" value={pct(u.cash_on_cash_pct)} />
        <Stat label="IRR" value={pct(u.irr_pct)} />
        <Stat
          label="DSCR"
          value={`${u.dscr.toFixed(2)}×`}
          tone={u.dscr >= 1.25 ? "good" : u.dscr >= 1 ? "warn" : "bad"}
        />
        <Stat
          label="Cash flow a year"
          value={u.annual_cash_flow_label}
          tone={u.annual_cash_flow_cents < 0 ? "bad" : undefined}
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <Panel>
          <PanelHeader
            title="Assumptions"
            description="Recalculate to see the numbers move. Save to keep them."
          />
          <form
            className="p-5 pt-4"
            onSubmit={(e) => {
              e.preventDefault();
              void recalc();
            }}
          >
            <fieldset disabled={!write} className="grid grid-cols-2 gap-3">
              {FIELDS.map((f) => (
                <F
                  key={f.k}
                  label={f.unit ? `${f.label} (${f.unit})` : f.label}
                >
                  <input
                    className={input}
                    value={form[f.k] ?? ""}
                    onChange={(e) => set(f.k)(e.target.value)}
                    inputMode={f.unit === "yrs" ? "numeric" : "decimal"}
                    placeholder={
                      f.k === "exit_cap_rate" ? "Blank uses appreciation" : ""
                    }
                  />
                </F>
              ))}
              <F label="Target close">
                <input
                  type="date"
                  className={input}
                  value={form.target_close_on ?? ""}
                  onChange={(e) => set("target_close_on")(e.target.value)}
                />
              </F>
              <F label="Notes" className="col-span-2">
                <textarea
                  rows={3}
                  className={input}
                  value={form.notes ?? ""}
                  onChange={(e) => set("notes")(e.target.value)}
                />
              </F>
            </fieldset>
            {write && (
              <div className="mt-4 flex gap-2">
                <Button
                  type="submit"
                  variant="secondary"
                  loading={busy === "recalc"}
                >
                  <Calculator />
                  Recalculate
                </Button>
                <Button type="button" onClick={save} loading={busy === "save"}>
                  <Save />
                  Save assumptions
                </Button>
              </div>
            )}
          </form>
        </Panel>

        <div className="space-y-4">
          <Panel>
            <PanelHeader title="Returns" />
            <dl className="grid gap-x-6 px-5 pt-3 pb-4 sm:grid-cols-2">
              {(
                [
                  ["All-in cost", u.total_project_cost_label],
                  ["Loan amount", u.loan_amount_label],
                  ["Cash invested", u.total_cash_invested_label],
                  ["Gross rent a year", u.gross_rent_annual_label],
                  ["Vacancy loss", `-${u.vacancy_loss_label}`],
                  [
                    "Operating expenses",
                    `-${u.operating_expenses_annual_label}`,
                  ],
                  ["Net operating income", u.noi_annual_label],
                  ["Debt service a year", `-${u.annual_debt_service_label}`],
                  ["Exit value", u.exit_value_label],
                  ["Loan payoff at exit", `-${u.loan_balance_at_exit_label}`],
                  ["Net sale proceeds", u.net_sale_proceeds_label],
                  ["Total profit over the hold", u.total_profit_label],
                ] as const
              ).map(([k, v]) => (
                <div
                  key={k}
                  className="flex items-baseline justify-between gap-3 border-b border-line/60 py-1.5 text-[13px]"
                >
                  <dt className="text-fg-3">{k}</dt>
                  <dd className="figure font-medium text-fg">{v}</dd>
                </div>
              ))}
            </dl>
          </Panel>
          <Panel>
            <PanelHeader
              title="Sensitivity"
              description="IRR as yearly rent growth moves two points either way."
            />
            <Sensitivity u={u} />
          </Panel>
        </div>
      </div>
    </section>
  );
}

function Sensitivity({ u }: { u: DealUnderwriting }) {
  const irrs = u.sensitivity
    .map((s) => s.irr_bps)
    .filter((b): b is number => b !== null);
  const max = irrs.length ? Math.max(...irrs, 1) : 1;
  return (
    <div className="space-y-2 px-5 pt-3 pb-5">
      {u.sensitivity.map((s) => {
        const w = s.irr_bps === null ? 0 : Math.max(0, (s.irr_bps / max) * 100);
        return (
          <div key={s.rent_growth_bps} className="flex items-center gap-3">
            <span className="w-24 shrink-0 text-xs text-fg-3">
              growth {pct(s.rent_growth_pct)}
            </span>
            <div className="h-2.5 flex-1 overflow-hidden rounded-full bg-fill">
              <div
                className="h-full rounded-full bg-accent"
                style={{ width: `${w}%` }}
              />
            </div>
            <span className="figure w-14 shrink-0 text-right text-xs font-medium text-fg">
              {pct(s.irr_pct)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

function Checklist({ deal, write }: { deal: DealDetail; write: boolean }) {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  const key = ["flips", "one", deal.id];

  async function save(list: typeof deal.checklist) {
    setBusy(true);
    try {
      const updated = await api.updateFlipChecklist(deal.id, list);
      qc.setQueryData<DealDetail>(key, (d) =>
        d ? { ...d, checklist: updated.checklist } : d
      );
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save the list");
    } finally {
      setBusy(false);
    }
  }

  const done = deal.checklist.filter((c) => c.done).length;
  return (
    <Panel>
      <PanelHeader
        title="Due diligence"
        description={
          deal.checklist.length
            ? `${done} of ${deal.checklist.length} done`
            : undefined
        }
      />
      <div className="p-5 pt-4">
        {deal.checklist.length === 0 ? (
          <EmptyState
            icon={<ClipboardCheck />}
            title="No checklist yet"
            className="py-6"
            action={
              write && (
                <Button
                  size="sm"
                  variant="secondary"
                  loading={busy}
                  onClick={() => save(DEFAULT_CHECKLIST)}
                >
                  Add the standard checklist
                </Button>
              )
            }
          />
        ) : (
          <ul className="space-y-1">
            {deal.checklist.map((c) => (
              <li key={c.key}>
                <label className="flex items-center gap-3 rounded-lg px-2 py-1.5 text-[13px] transition hover:bg-fill-2">
                  <input
                    type="checkbox"
                    checked={c.done}
                    disabled={!write || busy}
                    onChange={() =>
                      save(
                        deal.checklist.map((x) =>
                          x.key === c.key ? { ...x, done: !x.done } : x
                        )
                      )
                    }
                    className="size-4 accent-[var(--accent)]"
                  />
                  <span
                    className={c.done ? "text-fg-3 line-through" : "text-fg"}
                  >
                    {c.label}
                  </span>
                </label>
              </li>
            ))}
          </ul>
        )}
      </div>
    </Panel>
  );
}
