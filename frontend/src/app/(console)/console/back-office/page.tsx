"use client";

// The back office: a snapshot of the team and the money, then payroll (with
// Gusto), job profit, and the tax working file. Reading needs `team:read`;
// the money tabs need `payroll:read` and are hidden without it.

import { Suspense, useMemo, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import {
  ArrowDown,
  ArrowUp,
  Download,
  Lock,
  Printer,
  Send,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";
import {
  download,
  gusto,
  hm,
  isoDate,
  money,
  openPdf,
  pct,
  reports,
  weekStart,
  type Rollup,
  type WorkRow,
} from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import {
  decimalHours,
  errMsg,
  humanize,
  milesLabel,
  plural,
  useReady,
} from "@/lib/money-extra";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DataTable, Stat, Tabs } from "@/components/ui/data-table";
import { fieldClass, Input, Label } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

type Key = "overview" | "payroll" | "profit" | "taxes";

const ALL_TABS = [
  ["overview", "Overview"],
  ["payroll", "Payroll"],
  ["profit", "Profit"],
  ["taxes", "Taxes"],
] as const;

async function safe(fn: () => Promise<unknown>) {
  try {
    await fn();
  } catch (e) {
    toast.error(errMsg(e, "Couldn't get that file"));
  }
}

function lastFullWeek() {
  const mon = weekStart(new Date());
  mon.setDate(mon.getDate() - 7);
  const sun = new Date(mon);
  sun.setDate(sun.getDate() + 6);
  return { from: isoDate(mon), to: isoDate(sun) };
}

function thisMonth() {
  const d = new Date();
  return {
    from: isoDate(new Date(d.getFullYear(), d.getMonth(), 1)),
    to: isoDate(new Date(d.getFullYear(), d.getMonth() + 1, 0)),
  };
}

export default function BackOfficePage() {
  return (
    <Suspense fallback={<Skeleton className="h-64 rounded-2xl" />}>
      <BackOffice />
    </Suspense>
  );
}

function BackOffice() {
  const { can } = useAuth();
  const ready = useReady("team:read");
  const allowed = useReady();
  const payroll = can("payroll:read");
  const params = useSearchParams();
  const router = useRouter();
  const tabs = ALL_TABS.filter(([k]) => k === "overview" || payroll);
  const tab: Key =
    tabs.find(([k]) => k === params.get("tab"))?.[0] ?? "overview";
  const choose = (k: Key) =>
    router.replace(`/console/back-office?tab=${k}`, { scroll: false });

  if (allowed && !ready)
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Back office" title="Back office" />
        <Panel>
          <EmptyState
            icon={<Lock />}
            title="You don't have access to the back office"
            description="Ask an admin for the team:read permission."
          />
        </Panel>
      </div>
    );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Back office"
        title="Back office"
        description="The team, the clock and the money. Payroll, job profit and taxes in one place."
      />
      {tabs.length > 1 && <Tabs tabs={tabs} value={tab} onChange={choose} />}
      {tab === "overview" && <Overview ready={ready} />}
      {tab === "payroll" && <Payroll ready={ready} />}
      {tab === "profit" && <Profit ready={ready} />}
      {tab === "taxes" && <Taxes ready={ready} />}
    </div>
  );
}

// ---- shared -----------------------------------------------------------------

function LinkStat({
  href,
  ...props
}: React.ComponentProps<typeof Stat> & { href?: string }) {
  if (!href) return <Stat {...props} />;
  return (
    <Link
      href={href}
      className="block rounded-2xl transition duration-300 hover:-translate-y-px [&>div]:h-full [&>div]:hover:border-line-strong"
    >
      <Stat {...props} />
    </Link>
  );
}

function DateRange({
  from,
  to,
  setFrom,
  setTo,
}: {
  from: string;
  to: string;
  setFrom: (v: string) => void;
  setTo: (v: string) => void;
}) {
  return (
    <>
      <div className="space-y-1.5">
        <Label htmlFor="bo-from">From</Label>
        <input
          id="bo-from"
          type="date"
          className={cn(fieldClass, "block")}
          value={from}
          onChange={(e) => e.target.value && setFrom(e.target.value)}
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="bo-to">To</Label>
        <input
          id="bo-to"
          type="date"
          className={cn(fieldClass, "block")}
          value={to}
          min={from}
          onChange={(e) => e.target.value && setTo(e.target.value)}
        />
      </div>
    </>
  );
}

function LoadError({ error }: { error: Error }) {
  return (
    <Panel className="border-bad/30 p-4 text-[13px] text-bad">
      {error.message}
    </Panel>
  );
}

function LeftOut({ title, items }: { title: string; items: string[] }) {
  if (items.length === 0) return null;
  return (
    <div className="mx-5 mb-5 rounded-xl border border-warn/30 bg-warn/10 px-4 py-3 text-[13px]">
      <div className="flex items-center gap-2 font-medium text-warn">
        <TriangleAlert className="size-4" />
        {title}
      </div>
      <ul className="mt-1 list-disc pl-5 text-fg-2">
        {items.map((x, i) => (
          <li key={i}>{x}</li>
        ))}
      </ul>
    </div>
  );
}

// ---- overview ---------------------------------------------------------------

function Overview({ ready }: { ready: boolean }) {
  const q = useQuery({
    queryKey: ["backoffice", "dashboard"],
    queryFn: reports.dashboard,
    enabled: ready,
  });
  if (q.error) return <LoadError error={q.error} />;
  const d = q.data;
  if (!d)
    return (
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {Array.from({ length: 8 }, (_, i) => (
          <Skeleton key={i} className="h-[92px] rounded-2xl" />
        ))}
      </div>
    );
  return (
    <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
      <LinkStat
        label="On the clock now"
        value={d.clocked_in.length}
        hint={
          d.clocked_in.length ? d.clocked_in.join(", ") : "Nobody right now"
        }
        href="/console/timesheets"
        tone={d.clocked_in.length ? "good" : undefined}
      />
      <LinkStat label="Team size" value={d.team_size} href="/console/team" />
      <LinkStat
        label="Hours this week"
        value={hm(d.hours_this_week_minutes)}
        href="/console/timesheets"
      />
      <LinkStat
        label="Waiting approval"
        value={d.unapproved_entries}
        hint="time entries"
        href="/console/timesheets"
        tone={d.unapproved_entries ? "warn" : undefined}
      />
      <LinkStat
        label="Missed punches"
        value={d.missed_punches}
        hint="need a clock-out time"
        href="/console/timesheets"
        tone={d.missed_punches ? "bad" : undefined}
      />
      <LinkStat
        label="Time off requests"
        value={d.pending_time_off}
        hint="waiting on you"
        href="/console/team"
        tone={d.pending_time_off ? "warn" : undefined}
      />
      <LinkStat
        label="Expenses to pay back"
        value={money(d.expenses_to_reimburse_cents)}
        hint={plural(d.expenses_to_reimburse, "expense")}
        href="/console/expenses"
        tone={d.expenses_to_reimburse ? "warn" : undefined}
      />
      <LinkStat
        label="Approved time not billed"
        value={money(d.unbilled_time_cents)}
        hint={`on ${plural(d.work_orders_with_unbilled_time, "work order")}`}
        href="/console/timesheets"
      />
      {d.labor_cost_this_week_cents != null && (
        <LinkStat
          label="Labor cost this week"
          value={money(d.labor_cost_this_week_cents)}
          href="/console/timesheets"
        />
      )}
      <Stat
        label="Billed to owners this month"
        value={money(d.billed_to_owners_this_month_cents)}
      />
    </div>
  );
}

// ---- payroll and Gusto --------------------------------------------------------

function Payroll({ ready }: { ready: boolean }) {
  const [{ from: f0, to: t0 }] = useState(lastFullWeek);
  const [from, setFrom] = useState(f0);
  const [to, setTo] = useState(t0);
  const [approvedOnly, setApprovedOnly] = useState(true);
  const q = useQuery({
    queryKey: ["backoffice", "payroll", from, to, approvedOnly],
    queryFn: () => reports.payroll({ from, to, approved_only: approvedOnly }),
    enabled: ready,
    placeholderData: (prev) => prev,
  });
  const data = q.data;
  const stale = q.isPlaceholderData;
  const totals = useMemo(() => {
    const t = { days: 0, entries: 0, reg: 0, ot: 0, dt: 0 };
    for (const r of data?.rows ?? []) {
      t.days += r.days_worked;
      t.entries += r.entries;
      t.reg += r.regular_minutes;
      t.ot += r.overtime_minutes;
      t.dt += r.double_minutes;
    }
    return t;
  }, [data]);
  const exp = (format: "csv" | "pdf") =>
    reports.payrollExport({ from, to, approved_only: approvedOnly, format });

  return (
    <div className="space-y-5">
      <Panel className="flex flex-wrap items-end justify-between gap-3 p-4">
        <div className="flex flex-wrap items-end gap-3">
          <DateRange from={from} to={to} setFrom={setFrom} setTo={setTo} />
          <label className="flex items-center gap-2 pb-2.5 text-[13px] text-fg-2">
            <input
              type="checkbox"
              className="size-4 accent-[var(--accent)]"
              checked={approvedOnly}
              onChange={(e) => setApprovedOnly(e.target.checked)}
            />
            Approved time only
          </label>
        </div>
        <div className="flex gap-2">
          <Button
            size="sm"
            variant="secondary"
            onClick={() => void safe(() => openPdf(exp("pdf")))}
          >
            <Printer />
            Print
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() =>
              void safe(() => download(exp("csv"), `payroll-${from}-${to}.csv`))
            }
          >
            <Download />
            CSV
          </Button>
        </div>
      </Panel>

      {q.error && <LoadError error={q.error} />}
      {!data && !q.error && <Skeleton className="h-64 rounded-2xl" />}
      {data && (
        <Panel className={cn("overflow-hidden", stale && "opacity-60")}>
          <PanelHeader
            title="Payroll"
            description={`${data.from} to ${data.to} · ${
              data.approved_only ? "approved time only" : "all time entered"
            } · ${data.overtime_rule}`}
          />
          <DataTable
            className="mt-3"
            columns={[
              "Employee",
              "Week of",
              { label: "Days", num: true },
              { label: "Entries", num: true },
              { label: "Hours", num: true },
              { label: "Regular", num: true },
              { label: "OT 1.5x", num: true },
              { label: "DT 2x", num: true },
              { label: "Rate", num: true },
              { label: "Gross", num: true },
              { label: "Mileage paid back", num: true },
            ]}
            empty="No time in this period."
            rows={data.rows.map((r) => [
              r.name,
              r.week_of,
              r.days_worked,
              r.entries,
              decimalHours(r.minutes),
              decimalHours(r.regular_minutes),
              r.overtime_minutes ? decimalHours(r.overtime_minutes) : "—",
              r.double_minutes ? decimalHours(r.double_minutes) : "—",
              `${money(r.rate_cents)}/h`,
              money(r.gross_cents),
              r.mileage_paid_back_cents
                ? money(r.mileage_paid_back_cents)
                : "—",
            ])}
            totals={[
              "Total",
              "",
              totals.days,
              totals.entries,
              decimalHours(data.total_minutes),
              decimalHours(totals.reg),
              decimalHours(totals.ot),
              decimalHours(totals.dt),
              "",
              money(data.total_gross_cents),
              money(data.total_mileage_paid_back_cents),
            ]}
          />
          <div className="h-2" />
          <LeftOut title="Left out of this payroll" items={data.excluded} />
        </Panel>
      )}

      <GustoPanel from={from} to={to} ready={ready} />
    </div>
  );
}

function GustoPanel({
  from,
  to,
  ready,
}: {
  from: string;
  to: string;
  ready: boolean;
}) {
  const status = useQuery({
    queryKey: ["gusto", "status"],
    queryFn: gusto.status,
    enabled: ready,
  });
  const preview = useQuery({
    queryKey: ["gusto", "hours", from, to],
    queryFn: () => gusto.hours(from, to),
    enabled: ready,
  });
  const [payrollId, setPayrollId] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{
    pushed: string[];
    unmatched: string[];
    simulated: boolean;
  } | null>(null);

  async function push() {
    if (!payrollId.trim()) {
      toast.error("Paste the Gusto payroll ID first.");
      return;
    }
    setBusy(true);
    try {
      const res = await gusto.push(payrollId.trim(), from, to);
      setResult(res);
      if (res.simulated) {
        toast.info("Test mode. Nothing was sent to Gusto.", {
          description: `${plural(res.pushed.length, "person", "people")} would have been updated.`,
        });
      } else {
        toast.success(
          `Hours sent to Gusto for ${plural(res.pushed.length, "person", "people")}`
        );
      }
    } catch (e) {
      toast.error(errMsg(e, "Couldn't send the hours to Gusto"));
    } finally {
      setBusy(false);
    }
  }

  const s = status.data;
  return (
    <Panel className="overflow-hidden">
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            Send to Gusto
            {s &&
              (s.company_uuid == null ? (
                <Badge>not connected</Badge>
              ) : s.live ? (
                <Badge tone="good">connected</Badge>
              ) : (
                <Badge tone="warn">test mode</Badge>
              ))}
          </span>
        }
        description={`Hours for ${from} to ${to}, matched to Gusto by email. Pushing fills in a payroll you've started in Gusto. You still review and run it there.`}
        action={
          <Button
            size="sm"
            variant="secondary"
            onClick={() =>
              void safe(() =>
                download(
                  gusto.hoursCsvPath(from, to),
                  `gusto-hours-${from}.csv`
                )
              )
            }
          >
            <Download />
            Gusto CSV
          </Button>
        }
      />
      {preview.error && (
        <p className="px-5 pt-4 text-[13px] text-bad">
          {preview.error.message}
        </p>
      )}
      {preview.isLoading && <Skeleton className="m-5 h-32 rounded-xl" />}
      {preview.data && (
        <>
          <DataTable
            className="mt-3"
            columns={[
              "Name",
              "Email",
              { label: "Regular", num: true },
              { label: "OT", num: true },
              { label: "DT", num: true },
            ]}
            empty="No approved hours to send."
            rows={preview.data.lines.map((l) => [
              l.name,
              l.email,
              l.regular_hours,
              l.overtime_hours,
              l.double_overtime_hours,
            ])}
          />
          <div className="h-3" />
          <LeftOut title="Left out" items={preview.data.left_out} />
        </>
      )}

      <div className="flex flex-wrap items-end gap-2 border-t border-line p-5">
        <div className="min-w-[240px] flex-1 space-y-1.5">
          <Label htmlFor="gusto-payroll">Gusto payroll ID</Label>
          <Input
            id="gusto-payroll"
            placeholder="From the payroll's page in Gusto"
            value={payrollId}
            onChange={(e) => setPayrollId(e.target.value)}
          />
        </div>
        <Button
          loading={busy}
          disabled={!preview.data || preview.data.lines.length === 0}
          onClick={() => void push()}
        >
          <Send />
          Push hours
        </Button>
      </div>

      {result && (
        <div className="mx-5 mb-5 space-y-1 rounded-xl border border-line px-4 py-3 text-[13px] text-fg-2">
          {result.simulated && (
            <div className="font-medium text-warn">
              Test mode. Nothing was sent to Gusto.
            </div>
          )}
          <div>
            {result.simulated ? "Would have updated" : "Updated"}:{" "}
            {result.pushed.length ? result.pushed.join(", ") : "nobody"}
          </div>
          {result.unmatched.length > 0 && (
            <div className="text-warn">
              Not found in Gusto: {result.unmatched.join(", ")}
            </div>
          )}
        </div>
      )}
    </Panel>
  );
}

// ---- profit -----------------------------------------------------------------

type SortKey =
  | "title"
  | "property"
  | "category"
  | "minutes"
  | "labor_cents"
  | "parts_cents"
  | "extra"
  | "costs_cents"
  | "billed_cents"
  | "unbilled_cents"
  | "gross_cents"
  | "gross_bps"
  | "bill_rate_for_target_cents";

const TEXT_KEYS: SortKey[] = ["title", "property", "category"];

const WORK_COLS: { key: SortKey; label: string }[] = [
  { key: "title", label: "Work" },
  { key: "property", label: "Property" },
  { key: "category", label: "Category" },
  { key: "minutes", label: "Hours" },
  { key: "labor_cents", label: "Labor" },
  { key: "parts_cents", label: "Parts" },
  { key: "extra", label: "Mileage and exp." },
  { key: "costs_cents", label: "Costs" },
  { key: "billed_cents", label: "Billed" },
  { key: "unbilled_cents", label: "Unbilled" },
  { key: "gross_cents", label: "Gross" },
  { key: "gross_bps", label: "Gross %" },
  { key: "bill_rate_for_target_cents", label: "Rate for target" },
];

const ROLLUPS = [
  ["by_property", "By property"],
  ["by_technician", "By technician"],
  ["by_category", "By category"],
  ["by_month", "By month"],
] as const;
type RollupKey = (typeof ROLLUPS)[number][0];

const ROLLUP_FIRST: Record<RollupKey, string> = {
  by_property: "Property",
  by_technician: "Technician",
  by_category: "Category",
  by_month: "Month",
};

function sortValue(r: WorkRow, k: SortKey): string | number {
  if (k === "extra") return r.mileage_cents + r.expenses_cents;
  return r[k];
}

function Profit({ ready }: { ready: boolean }) {
  const [{ from: f0, to: t0 }] = useState(thisMonth);
  const [from, setFrom] = useState(f0);
  const [to, setTo] = useState(t0);
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({
    key: "gross_cents",
    desc: true,
  });
  const [rollup, setRollup] = useState<RollupKey>("by_property");
  const q = useQuery({
    queryKey: ["backoffice", "profit", from, to],
    queryFn: () => reports.profit({ from, to }),
    enabled: ready,
    placeholderData: (prev) => prev,
  });
  const data = q.data;

  const work = useMemo(() => {
    const rows = [...(data?.work ?? [])];
    rows.sort((a, b) => {
      const x = sortValue(a, sort.key);
      const y = sortValue(b, sort.key);
      const c =
        typeof x === "number" && typeof y === "number"
          ? x - y
          : String(x).localeCompare(String(y));
      return sort.desc ? -c : c;
    });
    return rows;
  }, [data, sort]);

  const exp = (format: "csv" | "pdf", section?: string) =>
    reports.profitExport({ from, to, format, section });
  const csv = (section: string) =>
    void safe(() =>
      download(exp("csv", section), `profit-${section}-${from}-${to}.csv`)
    );

  return (
    <div className="space-y-5">
      <Panel className="flex flex-wrap items-end justify-between gap-3 p-4">
        <div className="flex flex-wrap items-end gap-3">
          <DateRange from={from} to={to} setFrom={setFrom} setTo={setTo} />
        </div>
        <Button
          size="sm"
          variant="secondary"
          onClick={() => void safe(() => openPdf(exp("pdf")))}
        >
          <Printer />
          Print
        </Button>
      </Panel>

      {q.error && <LoadError error={q.error} />}
      {!data && !q.error && <Skeleton className="h-64 rounded-2xl" />}
      {data && (
        <div className={cn("space-y-5", q.isPlaceholderData && "opacity-60")}>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-5">
            <Stat
              label="Revenue"
              value={money(data.revenue_cents)}
              hint={`${money(data.revenue_cents - data.unbilled_cents)} billed, ${money(data.unbilled_cents)} not yet`}
            />
            <Stat label="Costs" value={money(data.costs_cents)} />
            <Stat
              label="Gross profit"
              value={money(data.gross_cents)}
              hint={`${pct(data.gross_bps)} margin · target ${pct(data.target_margin_bps)}`}
              tone={
                data.revenue_cents > 0 &&
                data.gross_bps < data.target_margin_bps
                  ? "warn"
                  : undefined
              }
            />
            <Stat label="Net" value={money(data.net_cents)} />
            <Stat label="Labor hours" value={hm(data.minutes)} />
            <Stat
              label="Revenue per labor hour"
              value={`${money(data.revenue_per_hour_cents)}/h`}
            />
            <Stat
              label="Not billed yet"
              value={money(data.unbilled_cents)}
              tone={data.unbilled_cents > 0 ? "warn" : undefined}
            />
            <Stat
              label="Outside vendor bills"
              value={money(data.vendor_bills_cents)}
            />
            <Stat
              label="Under target margin"
              value={data.under_target}
              hint={`below ${pct(data.target_margin_bps)}`}
              tone={data.under_target ? "warn" : undefined}
            />
          </div>

          <Panel className="overflow-hidden">
            <PanelHeader
              title="Work orders and projects"
              description="Press a heading to sort."
              action={
                <Button
                  size="sm"
                  variant="secondary"
                  onClick={() => csv("work-orders-projects")}
                >
                  <Download />
                  CSV
                </Button>
              }
            />
            <div className="mt-3 overflow-x-auto">
              <table className="w-full text-[13px]">
                <thead>
                  <tr className="border-b border-line text-left">
                    {WORK_COLS.map((c, j) => {
                      const on = sort.key === c.key;
                      return (
                        <th
                          key={c.key}
                          scope="col"
                          aria-sort={
                            on
                              ? sort.desc
                                ? "descending"
                                : "ascending"
                              : undefined
                          }
                          className={cn(
                            "px-4 py-2.5 whitespace-nowrap",
                            j > 2 && "text-right"
                          )}
                        >
                          <button
                            type="button"
                            onClick={() =>
                              setSort({
                                key: c.key,
                                desc: on
                                  ? !sort.desc
                                  : !TEXT_KEYS.includes(c.key),
                              })
                            }
                            className={cn(
                              "eyebrow inline-flex items-center gap-1 font-medium hover:text-fg",
                              on && "text-fg"
                            )}
                          >
                            {c.label}
                            {on &&
                              (sort.desc ? (
                                <ArrowDown className="size-3" />
                              ) : (
                                <ArrowUp className="size-3" />
                              ))}
                          </button>
                        </th>
                      );
                    })}
                  </tr>
                </thead>
                <tbody>
                  {work.length === 0 ? (
                    <tr>
                      <td
                        colSpan={WORK_COLS.length}
                        className="px-4 py-8 text-center text-fg-3"
                      >
                        No work in this period.
                      </td>
                    </tr>
                  ) : (
                    work.map((r) => (
                      <tr
                        key={`${r.kind}:${r.id}`}
                        className="border-b border-line/60 last:border-0 hover:bg-surface-2/60"
                      >
                        <td className="px-4 py-2.5 font-medium whitespace-nowrap text-fg">
                          {r.kind === "work_order" ? (
                            <Link
                              href={`/console/maintenance/${r.id}`}
                              className="hover:underline"
                            >
                              {r.title}
                            </Link>
                          ) : (
                            <>
                              {r.title}{" "}
                              <span className="text-xs font-normal text-fg-3">
                                project
                              </span>
                            </>
                          )}
                        </td>
                        <td className="px-4 py-2.5 whitespace-nowrap text-fg-2">
                          {r.property}
                        </td>
                        <td className="px-4 py-2.5 whitespace-nowrap text-fg-2">
                          {r.category}
                        </td>
                        {[
                          hm(r.minutes),
                          money(r.labor_cents),
                          money(r.parts_cents),
                          money(r.mileage_cents + r.expenses_cents),
                          money(r.costs_cents),
                          money(r.billed_cents),
                          r.unbilled_cents ? money(r.unbilled_cents) : "—",
                          money(r.gross_cents),
                        ].map((v, j) => (
                          <td
                            key={j}
                            className="px-4 py-2.5 text-right font-mono whitespace-nowrap text-fg-2 tabular-nums"
                          >
                            {v}
                          </td>
                        ))}
                        <td className="px-4 py-2.5 text-right font-mono whitespace-nowrap text-fg-2 tabular-nums">
                          {r.revenue_cents > 0 &&
                          r.gross_bps < data.target_margin_bps ? (
                            <Badge tone="warn">{pct(r.gross_bps)}</Badge>
                          ) : (
                            pct(r.gross_bps)
                          )}
                        </td>
                        <td className="px-4 py-2.5 text-right font-mono whitespace-nowrap text-fg-2 tabular-nums">
                          {r.bill_rate_for_target_cents
                            ? `${money(r.bill_rate_for_target_cents)}/h`
                            : "—"}
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </Panel>

          <Panel className="overflow-hidden">
            <div className="flex flex-wrap items-center justify-between gap-2 px-5 pt-5">
              <Tabs tabs={ROLLUPS} value={rollup} onChange={setRollup} />
              <Button
                size="sm"
                variant="secondary"
                onClick={() => csv(rollup.replace("_", "-"))}
              >
                <Download />
                CSV
              </Button>
            </div>
            <RollupTable
              rows={data[rollup]}
              target={data.target_margin_bps}
              first={ROLLUP_FIRST[rollup]}
            />
          </Panel>
        </div>
      )}
    </div>
  );
}

function RollupTable({
  rows,
  target,
  first,
}: {
  rows: Rollup[];
  target: number;
  first: string;
}) {
  return (
    <DataTable
      className="mt-3"
      columns={[
        first,
        { label: "Jobs", num: true },
        { label: "Hours", num: true },
        { label: "Revenue", num: true },
        { label: "Costs", num: true },
        { label: "Gross", num: true },
        { label: "Gross %", num: true },
        { label: "Revenue per hour", num: true },
      ]}
      empty="Nothing in this period."
      rows={rows.map((r) => [
        r.label,
        r.jobs,
        hm(r.minutes),
        money(r.revenue_cents),
        money(r.costs_cents),
        money(r.gross_cents),
        r.revenue_cents > 0 && r.gross_bps < target ? (
          <Badge key="g" tone="warn">
            {pct(r.gross_bps)}
          </Badge>
        ) : (
          pct(r.gross_bps)
        ),
        r.revenue_per_hour_cents ? `${money(r.revenue_per_hour_cents)}/h` : "—",
      ])}
    />
  );
}

// ---- taxes ------------------------------------------------------------------

const TAX_SECTIONS = [
  { section: "mileage-log", label: "Mileage log" },
  { section: "expense-ledger", label: "Expense ledger" },
  { section: "missing-receipts", label: "Missing receipts" },
  { section: "pay-by-person", label: "Pay by person" },
  { section: "expenses-by-category", label: "Expenses by category" },
];

function Taxes({ ready }: { ready: boolean }) {
  const [{ currentYear, today, soon }] = useState(() => {
    const d = new Date();
    const later = new Date(d);
    later.setDate(later.getDate() + 30);
    return {
      currentYear: d.getFullYear(),
      today: isoDate(d),
      soon: isoDate(later),
    };
  });
  const years = [currentYear, currentYear - 1, currentYear - 2];
  const [year, setYear] = useState(currentYear);
  const [quarter, setQuarter] = useState(0);
  const q = useQuery({
    queryKey: ["backoffice", "taxes", year, quarter],
    queryFn: () => reports.taxes({ year, quarter: quarter || undefined }),
    enabled: ready,
  });
  const data = q.data;
  const exp = (format: "csv" | "pdf", section?: string) =>
    reports.taxesExport({
      year,
      quarter: quarter || undefined,
      format,
      section,
    });
  const tag = `${year}${quarter ? `-q${quarter}` : ""}`;

  return (
    <div className="space-y-5">
      <Panel className="flex flex-wrap items-end justify-between gap-3 p-4">
        <div className="flex flex-wrap items-end gap-3">
          <div className="space-y-1.5">
            <Label htmlFor="tax-year">Year</Label>
            <select
              id="tax-year"
              className={cn(fieldClass, "block")}
              value={year}
              onChange={(e) => setYear(Number(e.target.value))}
            >
              {years.map((y) => (
                <option key={y} value={y}>
                  {y}
                </option>
              ))}
            </select>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="tax-period">Period</Label>
            <select
              id="tax-period"
              className={cn(fieldClass, "block")}
              value={quarter}
              onChange={(e) => setQuarter(Number(e.target.value))}
            >
              <option value={0}>Whole year</option>
              {[1, 2, 3, 4].map((n) => (
                <option key={n} value={n}>
                  Q{n}
                </option>
              ))}
            </select>
          </div>
        </div>
        <Button
          onClick={() =>
            void safe(() =>
              download(exp("pdf"), `accountant-package-${tag}.pdf`)
            )
          }
        >
          <Download />
          Accountant package (PDF)
        </Button>
      </Panel>

      <div className="flex flex-wrap items-center gap-2">
        <span className="eyebrow">CSV</span>
        {TAX_SECTIONS.map((s) => (
          <Button
            key={s.section}
            size="sm"
            variant="ghost"
            onClick={() =>
              void safe(() =>
                download(exp("csv", s.section), `${s.section}-${tag}.csv`)
              )
            }
          >
            <Download />
            {s.label}
          </Button>
        ))}
      </div>

      {q.error && <LoadError error={q.error} />}
      {!data && !q.error && <Skeleton className="h-64 rounded-2xl" />}
      {data && (
        <>
          <div className="text-[13px] text-fg-3">
            {data.label} · {data.from} to {data.to}
          </div>

          {data.missing_receipts > 0 && (
            <Panel className="flex flex-wrap items-center justify-between gap-3 border-warn/30 bg-warn/10 p-4 text-[13px]">
              <span className="flex items-center gap-2 text-fg-2">
                <TriangleAlert className="size-4 shrink-0 text-warn" />
                <span>
                  <strong className="text-warn">
                    {data.missing_receipts} deductible{" "}
                    {data.missing_receipts === 1
                      ? "expense has"
                      : "expenses have"}{" "}
                    no receipt.
                  </strong>{" "}
                  Add them before this goes to your accountant.
                </span>
              </span>
              <Button size="sm" variant="secondary" asChild>
                <Link href="/console/expenses">Go to expenses</Link>
              </Button>
            </Panel>
          )}

          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <Stat
              label="Deductible expenses"
              value={money(data.deductible_expenses_cents)}
            />
            <Stat
              label="Not deductible"
              value={money(data.nondeductible_expenses_cents)}
            />
            <Stat
              label="Mileage"
              value={money(data.mileage_cents)}
              hint={`${milesLabel(data.miles)} miles`}
            />
            <Stat
              label="Paid back to people"
              value={money(data.reimbursed_cents)}
            />
            <Stat label="W-2 wages" value={money(data.w2_gross_cents)} />
            <Stat
              label="Contractor pay (1099)"
              value={money(data.contractor_gross_cents)}
            />
            <Stat
              label="Billed to owners"
              value={money(data.billed_to_owners_cents)}
            />
            <LinkStat
              label="Missing receipts"
              value={data.missing_receipts}
              href="/console/expenses"
              tone={data.missing_receipts ? "warn" : undefined}
            />
          </div>

          <div className="grid gap-5 lg:grid-cols-[1fr_2fr]">
            <Panel className="overflow-hidden">
              <PanelHeader title="Expenses by category" />
              <DataTable
                className="mt-3"
                columns={["Category", { label: "Amount", num: true }]}
                empty="No expenses in this period."
                rows={data.by_category.map(([c, cents]) => [
                  humanize(c),
                  money(cents),
                ])}
                totals={[
                  "Total",
                  money(data.by_category.reduce((s, [, c]) => s + c, 0)),
                ]}
              />
            </Panel>
            <Panel className="overflow-hidden">
              <PanelHeader title="Pay by person" />
              <DataTable
                className="mt-3"
                columns={[
                  "Name",
                  "Form",
                  "Employment",
                  { label: "Hours", num: true },
                  { label: "OT", num: true },
                  { label: "DT", num: true },
                  { label: "Gross", num: true },
                  { label: "Mileage paid back", num: true },
                  { label: "Miles", num: true },
                ]}
                empty="No pay in this period."
                rows={data.pay_by_person.map((p) => [
                  p.name,
                  <Badge key="f" tone={p.form === "W-2" ? "info" : "accent"}>
                    {p.form}
                  </Badge>,
                  humanize(p.employment),
                  decimalHours(p.minutes),
                  p.overtime_minutes ? decimalHours(p.overtime_minutes) : "—",
                  p.double_minutes ? decimalHours(p.double_minutes) : "—",
                  money(p.gross_cents),
                  p.mileage_paid_back_cents
                    ? money(p.mileage_paid_back_cents)
                    : "—",
                  p.miles ? milesLabel(p.miles) : "—",
                ])}
              />
            </Panel>
          </div>

          <Panel>
            <PanelHeader title="Key dates" />
            <div className="p-5 pt-3">
              {data.key_dates.length === 0 ? (
                <p className="text-[13px] text-fg-3">
                  No dates for this period.
                </p>
              ) : (
                <ul className="space-y-1.5 text-[13px]">
                  {data.key_dates.map((k, i) => {
                    const upcoming = k.date >= today && k.date <= soon;
                    const past = k.date < today;
                    return (
                      <li
                        key={i}
                        className={cn(
                          "flex items-center gap-3 rounded-xl px-3 py-2",
                          upcoming
                            ? "border border-warn/30 bg-warn/10"
                            : "bg-fill",
                          past && "opacity-60"
                        )}
                      >
                        <span className="figure w-24 shrink-0 text-fg-2">
                          {k.date}
                        </span>
                        <span className="flex-1 text-fg">{k.what}</span>
                        {upcoming && <Badge tone="warn">coming up</Badge>}
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
          </Panel>

          <p className="text-xs text-fg-3">
            A working file for your accountant, not tax advice.
          </p>
        </>
      )}
    </div>
  );
}
