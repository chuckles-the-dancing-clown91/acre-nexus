"use client";

// The back office hub: a snapshot of the team and the money (overview), plus
// payroll (with Gusto), job profit and the tax working file. Gated by
// `team:read`; the money tabs need `payroll:read` and are hidden otherwise.

import { useCallback, useEffect, useMemo, useState } from "react";
import Link from "next/link";
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
  type BackOfficeDashboard,
  type GustoLine,
  type PayrollReport,
  type ProfitReport,
  type Rollup,
  type TaxReport,
  type WorkRow,
} from "@/lib/backoffice";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";

type TabKey = "overview" | "payroll" | "profit" | "taxes";

const TABS: { key: TabKey; label: string; payroll: boolean }[] = [
  { key: "overview", label: "Overview", payroll: false },
  { key: "payroll", label: "Payroll", payroll: true },
  { key: "profit", label: "Profit", payroll: true },
  { key: "taxes", label: "Taxes", payroll: true },
];

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";
const select =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";
const label = "flex flex-col gap-1 text-xs font-semibold text-ink-3";

function errMsg(e: unknown, fallback = "Something went wrong") {
  return e instanceof Error ? e.message : fallback;
}

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
  const { can } = useAuth();
  const read = can("team:read");
  const payroll = can("payroll:read");
  const [tab, setTab] = useState<TabKey>("overview");
  const tabs = TABS.filter((t) => !t.payroll || payroll);
  const active = tabs.some((t) => t.key === tab) ? tab : "overview";

  if (!read) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to the back office. Ask an admin for the{" "}
          <span className="font-mono">team:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Back office
        </h1>
        <p className="text-ink-3">
          The team, the clock and the money — payroll, job profit and taxes in
          one place.
        </p>
      </div>

      <div className="flex flex-wrap gap-1 border-b border-line">
        {tabs.map((t) => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={
              active === t.key
                ? "border-b-2 border-accent px-4 py-2 text-sm font-bold text-ink"
                : "px-4 py-2 text-sm font-semibold text-ink-3 hover:text-ink"
            }
          >
            {t.label}
          </button>
        ))}
      </div>

      {active === "overview" && <Overview />}
      {active === "payroll" && <Payroll />}
      {active === "profit" && <Profit />}
      {active === "taxes" && <Taxes />}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Shared bits
// ---------------------------------------------------------------------------

function Tile({
  label,
  value,
  sub,
  href,
  tone,
}: {
  label: string;
  value: string;
  sub?: React.ReactNode;
  href?: string;
  tone?: "warn" | "bad" | "good";
}) {
  const inner = (
    <Card
      className={`h-full p-4 ${href ? "transition hover:border-accent" : ""}`}
    >
      <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-ink-3">
        {label}
      </div>
      <div
        className={`font-display text-2xl font-extrabold tracking-tight ${
          tone === "warn"
            ? "text-warn"
            : tone === "bad"
              ? "text-bad"
              : tone === "good"
                ? "text-good"
                : ""
        }`}
      >
        {value}
      </div>
      {sub && <div className="mt-1 text-xs text-ink-3">{sub}</div>}
    </Card>
  );
  return href ? (
    <Link href={href} className="block">
      {inner}
    </Link>
  ) : (
    inner
  );
}

function Table({
  headers,
  rows,
  totals,
  empty = "Nothing here yet.",
  right = [],
}: {
  headers: React.ReactNode[];
  rows: React.ReactNode[][];
  totals?: React.ReactNode[];
  empty?: string;
  /** Column indexes to right-align. */
  right?: number[];
}) {
  const align = (j: number) => (right.includes(j) ? "text-right" : "");
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-line text-left text-xs uppercase tracking-wide text-ink-3">
            {headers.map((h, j) => (
              <th
                key={j}
                className={`whitespace-nowrap px-3 py-2 font-semibold ${align(j)}`}
              >
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.length === 0 ? (
            <tr>
              <td
                colSpan={headers.length}
                className="px-3 py-6 text-center text-ink-3"
              >
                {empty}
              </td>
            </tr>
          ) : (
            rows.map((row, i) => (
              <tr key={i} className="border-b border-line/60">
                {row.map((cell, j) => (
                  <td
                    key={j}
                    className={`whitespace-nowrap px-3 py-2 tabular-nums ${align(j)}`}
                  >
                    {cell}
                  </td>
                ))}
              </tr>
            ))
          )}
        </tbody>
        {totals && rows.length > 0 && (
          <tfoot>
            <tr className="border-t-2 border-line font-bold">
              {totals.map((cell, j) => (
                <td
                  key={j}
                  className={`whitespace-nowrap px-3 py-2 tabular-nums ${align(j)}`}
                >
                  {cell}
                </td>
              ))}
            </tr>
          </tfoot>
        )}
      </table>
    </div>
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
      <label className={label}>
        From
        <input
          type="date"
          className={select}
          value={from}
          onChange={(e) => e.target.value && setFrom(e.target.value)}
        />
      </label>
      <label className={label}>
        To
        <input
          type="date"
          className={select}
          value={to}
          onChange={(e) => e.target.value && setTo(e.target.value)}
        />
      </label>
    </>
  );
}

function Loading({ error }: { error: string | null }) {
  return error ? (
    <p className="text-bad">{error}</p>
  ) : (
    <p className="text-ink-3">Loading…</p>
  );
}

const hours = (m: number) =>
  (m / 60).toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });

// ---------------------------------------------------------------------------
// Overview
// ---------------------------------------------------------------------------

function Overview() {
  const [d, setD] = useState<BackOfficeDashboard | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    reports
      .dashboard()
      .then(setD)
      .catch((e) => setError(errMsg(e, "Couldn't load the dashboard")));
  }, []);

  if (!d) return <Loading error={error} />;

  return (
    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
      <Tile
        label="On the clock now"
        value={String(d.clocked_in.length)}
        sub={d.clocked_in.length ? d.clocked_in.join(", ") : "Nobody right now"}
        href="/console/timesheets"
        tone={d.clocked_in.length ? "good" : undefined}
      />
      <Tile
        label="Team size"
        value={String(d.team_size)}
        href="/console/team"
      />
      <Tile
        label="Hours this week"
        value={hm(d.hours_this_week_minutes)}
        href="/console/timesheets"
      />
      <Tile
        label="Waiting approval"
        value={String(d.unapproved_entries)}
        sub="time entries"
        href="/console/timesheets"
        tone={d.unapproved_entries ? "warn" : undefined}
      />
      <Tile
        label="Missed punches"
        value={String(d.missed_punches)}
        sub="need a clock-out time"
        href="/console/timesheets"
        tone={d.missed_punches ? "bad" : undefined}
      />
      <Tile
        label="Time off requests"
        value={String(d.pending_time_off)}
        sub="waiting on you"
        href="/console/team"
        tone={d.pending_time_off ? "warn" : undefined}
      />
      <Tile
        label="Expenses to pay back"
        value={money(d.expenses_to_reimburse_cents)}
        sub={`${d.expenses_to_reimburse} expense${d.expenses_to_reimburse === 1 ? "" : "s"}`}
        href="/console/expenses"
        tone={d.expenses_to_reimburse ? "warn" : undefined}
      />
      <Tile
        label="Approved time not billed"
        value={money(d.unbilled_time_cents)}
        sub={`on ${d.work_orders_with_unbilled_time} work order${d.work_orders_with_unbilled_time === 1 ? "" : "s"}`}
        href="/console/timesheets"
      />
      {d.labor_cost_this_week_cents != null && (
        <Tile
          label="Labor cost this week"
          value={money(d.labor_cost_this_week_cents)}
          href="/console/timesheets"
        />
      )}
      <Tile
        label="Billed to owners this month"
        value={money(d.billed_to_owners_this_month_cents)}
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Payroll + Gusto
// ---------------------------------------------------------------------------

function Payroll() {
  const [from, setFrom] = useState(() => lastFullWeek().from);
  const [to, setTo] = useState(() => lastFullWeek().to);
  const [approvedOnly, setApprovedOnly] = useState(true);
  const [data, setData] = useState<PayrollReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    reports
      .payroll({ from, to, approved_only: approvedOnly })
      .then((r) => {
        setData(r);
        setError(null);
      })
      .catch((e) => setError(errMsg(e, "Couldn't load payroll")));
  }, [from, to, approvedOnly]);

  const stale = data && (data.from !== from || data.to !== to);
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
      <Card className="flex flex-wrap items-end justify-between gap-3 p-4">
        <div className="flex flex-wrap items-end gap-3">
          <DateRange from={from} to={to} setFrom={setFrom} setTo={setTo} />
          <label className="flex items-center gap-2 pb-2 text-sm text-ink-2">
            <input
              type="checkbox"
              checked={approvedOnly}
              onChange={(e) => setApprovedOnly(e.target.checked)}
            />
            Approved time only
          </label>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            onClick={() => void safe(() => openPdf(exp("pdf")))}
          >
            Print
          </Button>
          <Button
            variant="outline"
            onClick={() =>
              void safe(() => download(exp("csv"), `payroll-${from}-${to}.csv`))
            }
          >
            CSV
          </Button>
        </div>
      </Card>

      {!data ? (
        <Loading error={error} />
      ) : (
        <Card className={`space-y-3 p-4 ${stale ? "opacity-60" : ""}`}>
          <div className="text-sm text-ink-3">
            {data.from} → {data.to} ·{" "}
            {data.approved_only ? "approved time only" : "all time entered"} ·{" "}
            {data.overtime_rule}
          </div>
          <Table
            headers={[
              "Employee",
              "Week of",
              "Days",
              "Entries",
              "Hours",
              "Regular",
              "OT 1.5×",
              "DT 2×",
              "Rate",
              "Gross",
              "Mileage paid back",
            ]}
            right={[2, 3, 4, 5, 6, 7, 8, 9, 10]}
            empty="No time in this period."
            rows={data.rows.map((r) => [
              <span key="n" className="font-semibold">
                {r.name}
              </span>,
              r.week_of,
              r.days_worked,
              r.entries,
              hours(r.minutes),
              hours(r.regular_minutes),
              r.overtime_minutes ? hours(r.overtime_minutes) : "—",
              r.double_minutes ? hours(r.double_minutes) : "—",
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
              hours(data.total_minutes),
              hours(totals.reg),
              hours(totals.ot),
              hours(totals.dt),
              "",
              money(data.total_gross_cents),
              money(data.total_mileage_paid_back_cents),
            ]}
          />
          {data.excluded.length > 0 && (
            <div className="rounded-xl border border-line-2 bg-warn-soft px-4 py-3 text-sm">
              <div className="font-semibold text-warn">
                Left out of this payroll
              </div>
              <ul className="mt-1 list-disc pl-5 text-ink-2">
                {data.excluded.map((x, i) => (
                  <li key={i}>{x}</li>
                ))}
              </ul>
            </div>
          )}
        </Card>
      )}

      <GustoPanel from={from} to={to} />
    </div>
  );
}

function GustoPanel({ from, to }: { from: string; to: string }) {
  const [status, setStatus] = useState<{
    company_uuid: string | null;
    live: boolean;
  } | null>(null);
  const [preview, setPreview] = useState<{
    from: string;
    to: string;
    lines: GustoLine[];
    left_out: string[];
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [payrollId, setPayrollId] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{
    pushed: string[];
    unmatched: string[];
    simulated: boolean;
  } | null>(null);

  useEffect(() => {
    gusto
      .status()
      .then(setStatus)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    gusto
      .hours(from, to)
      .then((p) => {
        setPreview(p);
        setError(null);
      })
      .catch((e) => setError(errMsg(e, "Couldn't load the Gusto hours")));
  }, [from, to]);

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
        toast.info("Test mode — nothing was sent to Gusto", {
          description: `${res.pushed.length} people would have been updated.`,
        });
      } else {
        toast.success(`Hours sent to Gusto for ${res.pushed.length} people`);
      }
    } catch (e) {
      toast.error(errMsg(e, "Couldn't send the hours to Gusto"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="space-y-4 p-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <h2 className="font-display text-lg font-bold">Send to Gusto</h2>
          {status &&
            (status.company_uuid == null ? (
              <Badge tone="neutral">not connected</Badge>
            ) : status.live ? (
              <Badge tone="good">connected</Badge>
            ) : (
              <Badge tone="warn">test mode</Badge>
            ))}
        </div>
        <Button
          variant="outline"
          onClick={() =>
            void safe(() =>
              download(gusto.hoursCsvPath(from, to), `gusto-hours-${from}.csv`)
            )
          }
        >
          Download Gusto CSV
        </Button>
      </div>
      <p className="text-sm text-ink-3">
        Hours for {from} → {to}, matched to Gusto by email. Pushing fills in the
        hours on a payroll you&apos;ve already started in Gusto — you still
        review and run it there.
      </p>

      {!preview ? (
        <Loading error={error} />
      ) : (
        <>
          <Table
            headers={["Name", "Email", "Regular", "OT", "DT"]}
            right={[2, 3, 4]}
            empty="No approved hours to send."
            rows={preview.lines.map((l) => [
              <span key="n" className="font-semibold">
                {l.name}
              </span>,
              l.email,
              l.regular_hours,
              l.overtime_hours,
              l.double_overtime_hours,
            ])}
          />
          {preview.left_out.length > 0 && (
            <div className="rounded-xl border border-line-2 bg-warn-soft px-4 py-3 text-sm">
              <div className="font-semibold text-warn">Left out</div>
              <ul className="mt-1 list-disc pl-5 text-ink-2">
                {preview.left_out.map((x, i) => (
                  <li key={i}>{x}</li>
                ))}
              </ul>
            </div>
          )}
        </>
      )}

      <div className="flex flex-wrap items-end gap-2">
        <label className={`${label} min-w-[260px] flex-1`}>
          Gusto payroll ID
          <input
            className={field}
            placeholder="From the payroll's page in Gusto"
            value={payrollId}
            onChange={(e) => setPayrollId(e.target.value)}
          />
        </label>
        <Button
          disabled={busy || !preview || preview.lines.length === 0}
          onClick={() => void push()}
        >
          {busy ? "Sending…" : "Push hours"}
        </Button>
      </div>

      {result && (
        <div className="space-y-1 rounded-xl border border-line px-4 py-3 text-sm">
          {result.simulated && (
            <div className="font-semibold text-warn">
              Test mode — nothing was sent to Gusto.
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
    </Card>
  );
}

// ---------------------------------------------------------------------------
// Profit
// ---------------------------------------------------------------------------

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

const ROLLUPS = [
  { key: "by_property", label: "By property", section: "by-property" },
  { key: "by_technician", label: "By technician", section: "by-technician" },
  { key: "by_category", label: "By category", section: "by-category" },
  { key: "by_month", label: "By month", section: "by-month" },
] as const;

const TEXT_KEYS: SortKey[] = ["title", "property", "category"];

function SortHead({
  k,
  sort,
  setSort,
  children,
}: {
  k: SortKey;
  sort: { key: SortKey; desc: boolean };
  setSort: (s: { key: SortKey; desc: boolean }) => void;
  children: React.ReactNode;
}) {
  const on = sort.key === k;
  return (
    <button
      className={`uppercase hover:text-ink ${on ? "text-ink" : ""}`}
      onClick={() =>
        setSort({
          key: k,
          desc: on ? !sort.desc : !TEXT_KEYS.includes(k),
        })
      }
    >
      {children}
      {on ? (sort.desc ? " ↓" : " ↑") : ""}
    </button>
  );
}

function sortValue(r: WorkRow, k: SortKey): string | number {
  if (k === "extra") return r.mileage_cents + r.expenses_cents;
  return r[k];
}

function Profit() {
  const [from, setFrom] = useState(() => thisMonth().from);
  const [to, setTo] = useState(() => thisMonth().to);
  const [data, setData] = useState<ProfitReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({
    key: "gross_cents",
    desc: true,
  });
  const [rollup, setRollup] =
    useState<(typeof ROLLUPS)[number]["key"]>("by_property");

  useEffect(() => {
    reports
      .profit({ from, to })
      .then((r) => {
        setData(r);
        setError(null);
      })
      .catch((e) => setError(errMsg(e, "Couldn't load the profit report")));
  }, [from, to]);

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

  const sp = { sort, setSort };
  const Th = SortHead;

  const csv = (section: string) =>
    void safe(() =>
      download(exp("csv", section), `profit-${section}-${from}-${to}.csv`)
    );

  const activeRollup = ROLLUPS.find((r) => r.key === rollup)!;

  return (
    <div className="space-y-5">
      <Card className="flex flex-wrap items-end justify-between gap-3 p-4">
        <div className="flex flex-wrap items-end gap-3">
          <DateRange from={from} to={to} setFrom={setFrom} setTo={setTo} />
        </div>
        <Button
          variant="outline"
          onClick={() => void safe(() => openPdf(exp("pdf")))}
        >
          Print
        </Button>
      </Card>

      {!data ? (
        <Loading error={error} />
      ) : (
        <>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-3 lg:grid-cols-5">
            <Tile
              label="Revenue"
              value={money(data.revenue_cents)}
              sub={`${money(data.revenue_cents - data.unbilled_cents)} billed + ${money(data.unbilled_cents)} not yet`}
            />
            <Tile label="Costs" value={money(data.costs_cents)} />
            <Tile
              label="Gross profit"
              value={money(data.gross_cents)}
              sub={`${pct(data.gross_bps)} margin · target ${pct(data.target_margin_bps)}`}
              tone={
                data.revenue_cents > 0 &&
                data.gross_bps < data.target_margin_bps
                  ? "warn"
                  : undefined
              }
            />
            <Tile label="Net" value={money(data.net_cents)} />
            <Tile label="Labor hours" value={hm(data.minutes)} />
            <Tile
              label="Revenue per labor hour"
              value={`${money(data.revenue_per_hour_cents)}/h`}
            />
            <Tile
              label="Not billed yet"
              value={money(data.unbilled_cents)}
              tone={data.unbilled_cents > 0 ? "warn" : undefined}
            />
            <Tile
              label="Outside vendor bills"
              value={money(data.vendor_bills_cents)}
            />
            <Tile
              label="Work under target margin"
              value={String(data.under_target)}
              sub={`below ${pct(data.target_margin_bps)}`}
              tone={data.under_target ? "warn" : undefined}
            />
          </div>

          <Card className="space-y-3 p-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h2 className="font-display text-lg font-bold">
                Work orders & projects
              </h2>
              <Button
                variant="outline"
                onClick={() => csv("work-orders-projects")}
              >
                CSV
              </Button>
            </div>
            <Table
              headers={[
                <Th key="t" k="title" {...sp}>
                  Work
                </Th>,
                <Th key="p" k="property" {...sp}>
                  Property
                </Th>,
                <Th key="c" k="category" {...sp}>
                  Category
                </Th>,
                <Th key="h" k="minutes" {...sp}>
                  Hours
                </Th>,
                <Th key="l" k="labor_cents" {...sp}>
                  Labor
                </Th>,
                <Th key="pa" k="parts_cents" {...sp}>
                  Parts
                </Th>,
                <Th key="m" k="extra" {...sp}>
                  Mileage + exp.
                </Th>,
                <Th key="co" k="costs_cents" {...sp}>
                  Costs
                </Th>,
                <Th key="b" k="billed_cents" {...sp}>
                  Billed
                </Th>,
                <Th key="u" k="unbilled_cents" {...sp}>
                  Unbilled
                </Th>,
                <Th key="g" k="gross_cents" {...sp}>
                  Gross
                </Th>,
                <Th key="gp" k="gross_bps" {...sp}>
                  Gross %
                </Th>,
                <Th key="r" k="bill_rate_for_target_cents" {...sp}>
                  Rate for target
                </Th>,
              ]}
              right={[3, 4, 5, 6, 7, 8, 9, 10, 11, 12]}
              empty="No work in this period."
              rows={work.map((r) => [
                r.kind === "work_order" ? (
                  <Link
                    key="t"
                    href={`/console/maintenance/${r.id}`}
                    className="font-semibold underline"
                  >
                    {r.title}
                  </Link>
                ) : (
                  <span key="t" className="font-semibold">
                    {r.title}{" "}
                    <span className="text-xs font-normal text-ink-3">
                      project
                    </span>
                  </span>
                ),
                r.property,
                r.category,
                hm(r.minutes),
                money(r.labor_cents),
                money(r.parts_cents),
                money(r.mileage_cents + r.expenses_cents),
                money(r.costs_cents),
                money(r.billed_cents),
                r.unbilled_cents ? money(r.unbilled_cents) : "—",
                money(r.gross_cents),
                <span key="gp" className="inline-flex items-center gap-1">
                  {r.revenue_cents > 0 &&
                  r.gross_bps < data.target_margin_bps ? (
                    <Badge tone="warn">{pct(r.gross_bps)}</Badge>
                  ) : (
                    pct(r.gross_bps)
                  )}
                </span>,
                r.bill_rate_for_target_cents
                  ? `${money(r.bill_rate_for_target_cents)}/h`
                  : "—",
              ])}
            />
          </Card>

          <Card className="space-y-3 p-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="flex flex-wrap gap-1">
                {ROLLUPS.map((r) => (
                  <button
                    key={r.key}
                    onClick={() => setRollup(r.key)}
                    className={`rounded-lg px-3 py-1.5 text-sm font-semibold ${
                      rollup === r.key
                        ? "bg-accent-soft text-accent-2"
                        : "text-ink-3 hover:bg-surface-2"
                    }`}
                  >
                    {r.label}
                  </button>
                ))}
              </div>
              <Button
                variant="outline"
                onClick={() => csv(activeRollup.section)}
              >
                CSV
              </Button>
            </div>
            <RollupTable
              rows={data[rollup]}
              target={data.target_margin_bps}
              first={
                rollup === "by_property"
                  ? "Property"
                  : rollup === "by_technician"
                    ? "Technician"
                    : rollup === "by_category"
                      ? "Category"
                      : "Month"
              }
            />
          </Card>
        </>
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
    <Table
      headers={[
        first,
        "Jobs",
        "Hours",
        "Revenue",
        "Costs",
        "Gross",
        "Gross %",
        "Revenue / h",
      ]}
      right={[1, 2, 3, 4, 5, 6, 7]}
      empty="Nothing in this period."
      rows={rows.map((r) => [
        <span key="l" className="font-semibold">
          {r.label}
        </span>,
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

// ---------------------------------------------------------------------------
// Taxes
// ---------------------------------------------------------------------------

const TAX_SECTIONS = [
  { section: "mileage-log", label: "Mileage log" },
  { section: "expense-ledger", label: "Expense ledger" },
  { section: "missing-receipts", label: "Missing receipts" },
  { section: "pay-by-person", label: "Pay by person" },
  { section: "expenses-by-category", label: "Expenses by category" },
];

function humanize(key: string): string {
  const s = key.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function Taxes() {
  const currentYear = new Date().getFullYear();
  const years = [currentYear, currentYear - 1, currentYear - 2];
  const [year, setYear] = useState(currentYear);
  const [quarter, setQuarter] = useState(0);
  const [data, setData] = useState<TaxReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    reports
      .taxes({ year, quarter: quarter || undefined })
      .then((r) => {
        setData(r);
        setError(null);
      })
      .catch((e) => setError(errMsg(e, "Couldn't load the tax report")));
  }, [year, quarter]);

  useEffect(() => {
    load();
  }, [load]);

  const exp = (format: "csv" | "pdf", section?: string) =>
    reports.taxesExport({
      year,
      quarter: quarter || undefined,
      format,
      section,
    });
  const tag = `${year}${quarter ? `-q${quarter}` : ""}`;

  const [{ today, soon }] = useState(() => {
    const d = new Date();
    const later = new Date(d);
    later.setDate(later.getDate() + 30);
    return { today: isoDate(d), soon: isoDate(later) };
  });

  return (
    <div className="space-y-5">
      <Card className="flex flex-wrap items-end justify-between gap-3 p-4">
        <div className="flex flex-wrap items-end gap-3">
          <label className={label}>
            Year
            <select
              className={select}
              value={year}
              onChange={(e) => setYear(Number(e.target.value))}
            >
              {years.map((y) => (
                <option key={y} value={y}>
                  {y}
                </option>
              ))}
            </select>
          </label>
          <label className={label}>
            Period
            <select
              className={select}
              value={quarter}
              onChange={(e) => setQuarter(Number(e.target.value))}
            >
              <option value={0}>Whole year</option>
              {[1, 2, 3, 4].map((q) => (
                <option key={q} value={q}>
                  Q{q}
                </option>
              ))}
            </select>
          </label>
        </div>
        <Button
          onClick={() =>
            void safe(() =>
              download(exp("pdf"), `accountant-package-${tag}.pdf`)
            )
          }
        >
          Download accountant package (PDF)
        </Button>
      </Card>

      <div className="flex flex-wrap items-center gap-2 text-sm">
        <span className="text-ink-3">CSV:</span>
        {TAX_SECTIONS.map((s) => (
          <button
            key={s.section}
            onClick={() =>
              void safe(() =>
                download(exp("csv", s.section), `${s.section}-${tag}.csv`)
              )
            }
            className="rounded-lg border border-line px-2.5 py-1 text-xs font-semibold text-ink-2 hover:border-accent"
          >
            {s.label}
          </button>
        ))}
      </div>

      {!data ? (
        <Loading error={error} />
      ) : (
        <>
          <div className="text-sm text-ink-3">
            {data.label} · {data.from} → {data.to}
          </div>

          {data.missing_receipts > 0 && (
            <Card className="flex flex-wrap items-center justify-between gap-3 border-line-2 bg-warn-soft p-4 text-sm">
              <span className="text-ink-2">
                <strong className="text-warn">
                  {data.missing_receipts} deductible expense
                  {data.missing_receipts === 1 ? " has" : "s have"} no receipt.
                </strong>{" "}
                Add them before handing this to your accountant.
              </span>
              <Link
                href="/console/expenses"
                className="font-semibold text-ink underline"
              >
                Go to expenses
              </Link>
            </Card>
          )}

          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <Tile
              label="Deductible expenses"
              value={money(data.deductible_expenses_cents)}
            />
            <Tile
              label="Not deductible"
              value={money(data.nondeductible_expenses_cents)}
            />
            <Tile
              label="Mileage"
              value={money(data.mileage_cents)}
              sub={`${data.miles.toLocaleString("en-US", { maximumFractionDigits: 1 })} miles`}
            />
            <Tile
              label="Paid back to people"
              value={money(data.reimbursed_cents)}
            />
            <Tile label="W-2 wages" value={money(data.w2_gross_cents)} />
            <Tile
              label="Contractor pay (1099)"
              value={money(data.contractor_gross_cents)}
            />
            <Tile
              label="Billed to owners"
              value={money(data.billed_to_owners_cents)}
            />
            <Tile
              label="Missing receipts"
              value={String(data.missing_receipts)}
              href="/console/expenses"
              tone={data.missing_receipts ? "warn" : undefined}
            />
          </div>

          <div className="grid gap-5 lg:grid-cols-[1fr_2fr]">
            <Card className="space-y-3 p-4">
              <h2 className="font-display text-lg font-bold">
                Expenses by category
              </h2>
              <Table
                headers={["Category", "Amount"]}
                right={[1]}
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
            </Card>

            <Card className="space-y-3 p-4">
              <h2 className="font-display text-lg font-bold">Pay by person</h2>
              <Table
                headers={[
                  "Name",
                  "Form",
                  "Employment",
                  "Hours",
                  "OT",
                  "DT",
                  "Gross",
                  "Mileage paid back",
                  "Miles",
                ]}
                right={[3, 4, 5, 6, 7, 8]}
                empty="No pay in this period."
                rows={data.pay_by_person.map((p) => [
                  <span key="n" className="font-semibold">
                    {p.name}
                  </span>,
                  <Badge key="f" tone={p.form === "W-2" ? "info" : "accent"}>
                    {p.form}
                  </Badge>,
                  humanize(p.employment),
                  hours(p.minutes),
                  p.overtime_minutes ? hours(p.overtime_minutes) : "—",
                  p.double_minutes ? hours(p.double_minutes) : "—",
                  money(p.gross_cents),
                  p.mileage_paid_back_cents
                    ? money(p.mileage_paid_back_cents)
                    : "—",
                  p.miles
                    ? p.miles.toLocaleString("en-US", {
                        maximumFractionDigits: 1,
                      })
                    : "—",
                ])}
              />
            </Card>
          </div>

          <Card className="space-y-3 p-4">
            <h2 className="font-display text-lg font-bold">Key dates</h2>
            {data.key_dates.length === 0 ? (
              <p className="text-sm text-ink-3">No dates for this period.</p>
            ) : (
              <ul className="space-y-1.5 text-sm">
                {data.key_dates.map((k, i) => {
                  const upcoming = k.date >= today && k.date <= soon;
                  const past = k.date < today;
                  return (
                    <li
                      key={i}
                      className={`flex items-center gap-3 rounded-xl px-3 py-2 ${
                        upcoming
                          ? "border border-line-2 bg-warn-soft"
                          : "bg-surface-2"
                      } ${past ? "opacity-60" : ""}`}
                    >
                      <span className="w-24 shrink-0 font-mono">{k.date}</span>
                      <span className="flex-1">{k.what}</span>
                      {upcoming && <Badge tone="warn">coming up</Badge>}
                    </li>
                  );
                })}
              </ul>
            )}
          </Card>

          <p className="text-xs text-ink-3">
            A working file for your accountant, not tax advice.
          </p>
        </>
      )}
    </div>
  );
}
