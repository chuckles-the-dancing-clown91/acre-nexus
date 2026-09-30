"use client";

// Timesheets: everyone's time for a week (or any range), with approval,
// missed-punch settling, fixes, and PDF / CSV export. Gated by `team:read`;
// changes need `team:manage`. Rates and labor cost only come back with
// `payroll:read`.

import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import type { MaintenanceTicket, Property } from "@/lib/types";
import {
  ENTRY_KIND_LABELS,
  download,
  hm,
  isoDate,
  localToRfc3339,
  money,
  reports,
  rfc3339ToLocal,
  team,
  weekStart,
  type Employee,
  type EntryInput,
  type EntryKind,
  type PersonHours,
  type Target,
  type TimeEntry,
} from "@/lib/backoffice";
import { Badge, Button, Card } from "@/components/ui";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-sm outline-none focus:border-accent";
const label = "flex flex-col gap-1 text-xs font-semibold text-ink-3";

function errMsg(e: unknown, fallback = "Something went wrong") {
  return e instanceof Error ? e.message : fallback;
}

function addDays(d: Date, n: number) {
  const x = new Date(d);
  x.setDate(x.getDate() + n);
  return x;
}

function parseYmd(s: string) {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(y, m - 1, d);
}

function fmtDay(iso: string) {
  return new Date(iso).toLocaleDateString([], {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
}

function fmtTime(iso: string | null) {
  if (!iso) return "—";
  return new Date(iso).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

function fmtWhen(iso: string) {
  return `${fmtDay(iso)} ${fmtTime(iso)}`;
}

function distance(m: number | null) {
  if (m == null) return "unknown";
  return m >= 160
    ? `${(m / 1609.34).toFixed(1)} mi`
    : `${Math.round(m * 3.281)} ft`;
}

function workLabel(e: TimeEntry) {
  if (e.kind === "work_order")
    return e.work_order_title ?? ENTRY_KIND_LABELS.work_order;
  if (e.kind === "project") return e.project_name ?? ENTRY_KIND_LABELS.project;
  if (e.kind === "property")
    return e.property_name ?? ENTRY_KIND_LABELS.property;
  return ENTRY_KIND_LABELS[e.kind];
}

function isOpen(e: TimeEntry) {
  return !e.ended_at && !e.needs_review;
}

type Status = "" | "unapproved" | "missed" | "open" | "approved";
const STATUS_TABS: [Status, string][] = [
  ["", "All"],
  ["unapproved", "Waiting approval"],
  ["missed", "Missed punches"],
  ["open", "On the clock"],
  ["approved", "Approved"],
];

const CLOSED_TICKETS = new Set(["resolved", "closed", "cancelled"]);

export default function TimesheetsPage() {
  const { can } = useAuth();
  const read = can("team:read");
  const manage = can("team:manage");

  const [from, setFrom] = useState(() => isoDate(weekStart(new Date())));
  const [to, setTo] = useState(() =>
    isoDate(addDays(weekStart(new Date()), 6))
  );
  const [custom, setCustom] = useState(false);
  const [userId, setUserId] = useState("");
  const [status, setStatus] = useState<Status>("");

  const [roster, setRoster] = useState<Employee[]>([]);
  const [tickets, setTickets] = useState<MaintenanceTicket[]>([]);
  const [properties, setProperties] = useState<Property[]>([]);
  const [entries, setEntries] = useState<TimeEntry[]>([]);
  const [summary, setSummary] = useState<PersonHours[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [editing, setEditing] = useState<TimeEntry | null>(null);
  const [adding, setAdding] = useState(false);
  const [settling, setSettling] = useState<TimeEntry | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    team
      .time({
        from,
        to,
        user_id: userId || undefined,
        status: status || undefined,
      })
      .then((list) => {
        setEntries(list);
        setSelected(new Set());
        setLoaded(true);
        setError(null);
      })
      .catch((e) => setError(errMsg(e)));
    team
      .summary({ from, to })
      .then(setSummary)
      .catch(() => undefined);
  }, [from, to, userId, status]);

  useEffect(() => {
    if (!read) return;
    load();
  }, [read, load]);

  useEffect(() => {
    if (!read) return;
    team
      .roster()
      .then(setRoster)
      .catch(() => undefined);
    if (!manage) return;
    api
      .tickets()
      .then((t) => setTickets(t.filter((x) => !CLOSED_TICKETS.has(x.status))))
      .catch(() => undefined);
    api
      .properties()
      .then(setProperties)
      .catch(() => undefined);
  }, [read, manage]);

  const people = useMemo(() => roster.filter((p) => p.profile), [roster]);

  if (!read) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to timesheets. Ask an admin for the{" "}
          <span className="font-mono">team:read</span> permission.
        </p>
      </Card>
    );
  }

  function shiftWeek(n: number) {
    const start = addDays(weekStart(parseYmd(from)), n * 7);
    setFrom(isoDate(start));
    setTo(isoDate(addDays(start, 6)));
    setCustom(false);
  }

  function thisWeek() {
    const start = weekStart(new Date());
    setFrom(isoDate(start));
    setTo(isoDate(addDays(start, 6)));
    setCustom(false);
  }

  async function run(fn: () => Promise<unknown>, ok: string, fail: string) {
    setBusy(true);
    try {
      await fn();
      toast.success(ok);
      load();
    } catch (e) {
      toast.error(errMsg(e, fail));
    } finally {
      setBusy(false);
    }
  }

  async function approveSelected() {
    const ids = [...selected];
    if (ids.length === 0) return;
    setBusy(true);
    try {
      const res = await team.approveMany(ids);
      if (res.skipped.length === 0) {
        toast.success(
          `Approved ${res.approved} entr${res.approved === 1 ? "y" : "ies"}`
        );
      } else {
        const reasons = [...new Set(res.skipped.map((s) => s.reason))];
        toast.warning(
          `Approved ${res.approved}, skipped ${res.skipped.length}`,
          { description: reasons.join(" · ") }
        );
      }
      load();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't approve those"));
    } finally {
      setBusy(false);
    }
  }

  async function exportAs(format: "pdf" | "csv") {
    try {
      await download(
        reports.timesheetsExport({
          from,
          to,
          user_id: userId || undefined,
          format,
        }),
        `timesheets-${from}-to-${to}.${format}`
      );
    } catch (e) {
      toast.error(errMsg(e, "Couldn't download the timesheets"));
    }
  }

  const selectable = entries.filter((e) => !e.approved && !isOpen(e));
  const allSelected =
    selectable.length > 0 && selectable.every((e) => selected.has(e.id));
  const totalMinutes = entries.reduce((s, e) => s + e.minutes, 0);
  const seeCost = entries.some((e) => e.labor_cost_cents != null);
  const summaryRows = userId
    ? summary.filter((s) => s.user_id === userId)
    : summary;

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Timesheets
          </h1>
          <p className="text-ink-3">
            Check and approve everyone&apos;s hours before payroll and owner
            billing.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="outline" onClick={() => exportAs("pdf")}>
            Download PDF
          </Button>
          <Button variant="outline" onClick={() => exportAs("csv")}>
            CSV
          </Button>
          {manage && <Button onClick={() => setAdding(true)}>Add time</Button>}
        </div>
      </div>

      <Card className="flex flex-wrap items-end gap-3 p-4">
        {custom ? (
          <>
            <label className={label}>
              From
              <input
                className={field}
                type="date"
                value={from}
                onChange={(e) => e.target.value && setFrom(e.target.value)}
              />
            </label>
            <label className={label}>
              To
              <input
                className={field}
                type="date"
                value={to}
                min={from}
                onChange={(e) => e.target.value && setTo(e.target.value)}
              />
            </label>
            <Button variant="ghost" onClick={thisWeek}>
              Back to weeks
            </Button>
          </>
        ) : (
          <div className="flex items-center gap-2">
            <Button variant="outline" onClick={() => shiftWeek(-1)}>
              ←
            </Button>
            <div className="min-w-44 text-center">
              <div className="text-xs font-semibold text-ink-3">Week of</div>
              <div className="font-semibold">
                {parseYmd(from).toLocaleDateString([], {
                  month: "short",
                  day: "numeric",
                })}{" "}
                –{" "}
                {parseYmd(to).toLocaleDateString([], {
                  month: "short",
                  day: "numeric",
                  year: "numeric",
                })}
              </div>
            </div>
            <Button variant="outline" onClick={() => shiftWeek(1)}>
              →
            </Button>
            <Button variant="ghost" onClick={thisWeek}>
              This week
            </Button>
            <Button variant="ghost" onClick={() => setCustom(true)}>
              Custom dates
            </Button>
          </div>
        )}
        <label className={`${label} ml-auto w-56`}>
          Person
          <select
            className={field}
            value={userId}
            onChange={(e) => setUserId(e.target.value)}
          >
            <option value="">Everyone</option>
            {people.map((p) => (
              <option key={p.user_id} value={p.user_id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
      </Card>

      {summaryRows.length > 0 && (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {summaryRows.map((s) => (
            <button
              key={s.user_id}
              onClick={() => setUserId(userId === s.user_id ? "" : s.user_id)}
              className="text-left"
            >
              <Card
                className={`p-4 transition hover:border-accent ${
                  userId === s.user_id ? "border-accent" : ""
                }`}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate font-semibold">{s.name}</span>
                  {s.clocked_in && <Badge tone="good">on</Badge>}
                </div>
                <div className="mt-1 font-display text-2xl font-extrabold">
                  {hm(s.minutes)}
                </div>
                <div className="mt-1 space-y-0.5 text-xs text-ink-3">
                  <div>
                    {hm(s.on_work_minutes)} on work · {hm(s.other_minutes)}{" "}
                    other
                  </div>
                  {(s.unapproved_minutes > 0 || s.missed_punches > 0) && (
                    <div className="text-warn">
                      {s.unapproved_minutes > 0 &&
                        `${hm(s.unapproved_minutes)} waiting approval`}
                      {s.unapproved_minutes > 0 &&
                        s.missed_punches > 0 &&
                        " · "}
                      {s.missed_punches > 0 &&
                        `${s.missed_punches} missed punch${s.missed_punches === 1 ? "" : "es"}`}
                    </div>
                  )}
                  {s.labor_cost_cents != null && (
                    <div>Labor cost {money(s.labor_cost_cents)}</div>
                  )}
                </div>
              </Card>
            </button>
          ))}
        </div>
      )}

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-1">
          {STATUS_TABS.map(([k, name]) => (
            <button
              key={k || "all"}
              onClick={() => setStatus(k)}
              className={`rounded-lg px-3 py-1.5 text-sm font-semibold ${
                status === k
                  ? "bg-accent-soft text-accent-2"
                  : "text-ink-3 hover:bg-surface-2"
              }`}
            >
              {name}
            </button>
          ))}
        </div>
        {manage && (
          <Button
            onClick={approveSelected}
            disabled={busy || selected.size === 0}
          >
            Approve selected{selected.size > 0 && ` (${selected.size})`}
          </Button>
        )}
      </div>

      {error && (
        <Card className="p-4 text-sm text-bad">
          <p role="alert">{error}</p>
        </Card>
      )}

      <Card className="overflow-x-auto">
        <table className="w-full min-w-[1000px] text-sm">
          <thead className="text-left text-xs font-bold uppercase tracking-wide text-ink-3">
            <tr className="border-b border-line">
              {manage && (
                <th className="w-10 px-4 py-3">
                  <input
                    type="checkbox"
                    aria-label="Select all"
                    checked={allSelected}
                    disabled={selectable.length === 0}
                    onChange={(e) =>
                      setSelected(
                        e.target.checked
                          ? new Set(selectable.map((x) => x.id))
                          : new Set()
                      )
                    }
                  />
                </th>
              )}
              <th className="px-3 py-3">Date</th>
              <th className="px-3 py-3">Person</th>
              <th className="px-3 py-3">Work</th>
              <th className="px-3 py-3">In</th>
              <th className="px-3 py-3">Out</th>
              <th className="px-3 py-3 text-right">Break</th>
              <th className="px-3 py-3 text-right">Hours</th>
              {seeCost && <th className="px-3 py-3 text-right">Cost</th>}
              <th className="px-3 py-3" />
              {manage && <th className="px-4 py-3" />}
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {entries.map((e) => (
              <tr
                key={e.id}
                className={selected.has(e.id) ? "bg-accent-soft/30" : ""}
              >
                {manage && (
                  <td className="px-4 py-3">
                    {!e.approved && !isOpen(e) && (
                      <input
                        type="checkbox"
                        aria-label="Select"
                        checked={selected.has(e.id)}
                        onChange={(ev) => {
                          const next = new Set(selected);
                          if (ev.target.checked) next.add(e.id);
                          else next.delete(e.id);
                          setSelected(next);
                        }}
                      />
                    )}
                  </td>
                )}
                <td className="whitespace-nowrap px-3 py-3">
                  {fmtDay(e.started_at)}
                </td>
                <td className="px-3 py-3 font-semibold">{e.user_name}</td>
                <td className="px-3 py-3">
                  <div>{workLabel(e)}</div>
                  <div className="text-xs text-ink-3">
                    {ENTRY_KIND_LABELS[e.kind]}
                    {e.property_name && e.kind !== "property" && (
                      <> · {e.property_name}</>
                    )}
                    {e.notes && <> · {e.notes}</>}
                  </div>
                </td>
                <td className="whitespace-nowrap px-3 py-3">
                  {fmtTime(e.started_at)}
                </td>
                <td className="whitespace-nowrap px-3 py-3">
                  {isOpen(e) ? (
                    <span className="text-good">still on</span>
                  ) : (
                    fmtTime(e.ended_at)
                  )}
                </td>
                <td className="px-3 py-3 text-right">
                  {e.break_minutes ? `${e.break_minutes}m` : "—"}
                </td>
                <td className="px-3 py-3 text-right font-semibold tabular-nums">
                  {hm(e.minutes)}
                </td>
                {seeCost && (
                  <td
                    className="px-3 py-3 text-right tabular-nums"
                    title={
                      e.pay_rate_cents != null
                        ? `Pay ${money(e.pay_rate_cents)}/h${
                            e.bill_rate_cents != null
                              ? ` · bills ${money(e.bill_rate_cents)}/h`
                              : ""
                          }`
                        : undefined
                    }
                  >
                    {e.labor_cost_cents != null
                      ? money(e.labor_cost_cents)
                      : "—"}
                  </td>
                )}
                <td className="px-3 py-3">
                  <div className="flex flex-wrap gap-1">
                    {isOpen(e) && <Badge tone="accent">on the clock</Badge>}
                    {e.approved && <Badge tone="good">approved</Badge>}
                    {e.billed && <Badge tone="info">billed · locked</Badge>}
                    {(e.missed_punch || e.needs_review) && (
                      <span
                        title={[
                          e.missed_punch_reason,
                          e.claimed_end &&
                            `They say they finished ${fmtWhen(e.claimed_end)}`,
                          e.punch_note && `“${e.punch_note}”`,
                        ]
                          .filter(Boolean)
                          .join(" · ")}
                      >
                        <Badge tone="warn">
                          missed punch
                          {e.needs_review && e.claimed_end && " · claimed"}
                        </Badge>
                      </span>
                    )}
                    {!e.approved && !isOpen(e) && !e.needs_review && (
                      <Badge>waiting</Badge>
                    )}
                    {e.away && (
                      <span
                        title={`Clocked in ${distance(e.in_distance_m)} from the job, out ${distance(e.out_distance_m)} from the job`}
                      >
                        <Badge tone="bad">away</Badge>
                      </span>
                    )}
                  </div>
                  {e.needs_review && (
                    <div className="mt-1 text-xs text-ink-3">
                      {e.missed_punch_reason}
                      {e.claimed_end && (
                        <> · says done {fmtWhen(e.claimed_end)}</>
                      )}
                      {e.punch_note && <> · “{e.punch_note}”</>}
                    </div>
                  )}
                </td>
                {manage && (
                  <td className="whitespace-nowrap px-4 py-3 text-right">
                    <div className="flex justify-end gap-1">
                      {e.needs_review && (
                        <RowButton onClick={() => setSettling(e)}>
                          Settle
                        </RowButton>
                      )}
                      {isOpen(e) && (
                        <RowButton
                          disabled={busy}
                          onClick={() =>
                            run(
                              () => team.clockOut(e.id),
                              `Clocked ${e.user_name} out`,
                              "Couldn't clock them out"
                            )
                          }
                        >
                          Clock out now
                        </RowButton>
                      )}
                      {!e.approved && !isOpen(e) && !e.needs_review && (
                        <RowButton
                          disabled={busy}
                          onClick={() =>
                            run(
                              () => team.approve(e.id),
                              "Approved",
                              "Couldn't approve that"
                            )
                          }
                        >
                          Approve
                        </RowButton>
                      )}
                      <RowButton
                        disabled={e.billed}
                        title={
                          e.billed
                            ? "Already billed to the owner — this time is locked"
                            : undefined
                        }
                        onClick={() => setEditing(e)}
                      >
                        Edit
                      </RowButton>
                      <RowButton
                        disabled={busy || e.billed}
                        title={
                          e.billed
                            ? "Already billed to the owner — this time is locked"
                            : undefined
                        }
                        danger
                        onClick={() => {
                          if (!confirm(`Delete this time for ${e.user_name}?`))
                            return;
                          run(
                            () => team.deleteTime(e.id),
                            "Time deleted",
                            "Couldn't delete that"
                          );
                        }}
                      >
                        Delete
                      </RowButton>
                    </div>
                  </td>
                )}
              </tr>
            ))}
            {!loaded && !error && (
              <tr>
                <td colSpan={12} className="px-5 py-10 text-center text-ink-3">
                  Loading…
                </td>
              </tr>
            )}
            {loaded && entries.length === 0 && (
              <tr>
                <td colSpan={12} className="px-5 py-10 text-center text-ink-3">
                  No time here.
                </td>
              </tr>
            )}
          </tbody>
          {entries.length > 0 && (
            <tfoot>
              <tr className="border-t border-line font-semibold">
                <td
                  colSpan={manage ? 8 : 7}
                  className="px-3 py-3 text-right text-ink-3"
                >
                  {entries.length} entr{entries.length === 1 ? "y" : "ies"}
                </td>
                <td className="px-3 py-3 text-right tabular-nums">
                  {hm(totalMinutes)}
                </td>
                <td colSpan={4} />
              </tr>
            </tfoot>
          )}
        </table>
      </Card>

      <Dialog
        open={adding || !!editing}
        onOpenChange={(o) => {
          if (!o) {
            setAdding(false);
            setEditing(null);
          }
        }}
      >
        <DialogContent className="max-h-[90vh] overflow-y-auto">
          {(adding || editing) && (
            <EntryForm
              key={editing?.id ?? "new"}
              entry={editing}
              people={people}
              defaultUser={userId}
              tickets={tickets}
              properties={properties}
              onDone={() => {
                setAdding(false);
                setEditing(null);
                load();
              }}
            />
          )}
        </DialogContent>
      </Dialog>

      <Dialog open={!!settling} onOpenChange={(o) => !o && setSettling(null)}>
        <DialogContent>
          {settling && (
            <SettleForm
              key={settling.id}
              entry={settling}
              onDone={() => {
                setSettling(null);
                load();
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}

function RowButton({
  children,
  danger,
  ...rest
}: {
  children: React.ReactNode;
  danger?: boolean;
} & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      {...rest}
      className={`rounded-lg border border-line px-2 py-1 text-xs font-semibold text-ink-2 disabled:opacity-40 ${
        danger ? "hover:border-bad hover:text-bad" : "hover:border-accent"
      }`}
    >
      {children}
    </button>
  );
}

// ---- add / edit -------------------------------------------------------------

const OTHER_KINDS: EntryKind[] = ["travel", "shop", "admin", "other"];

function targetValue(t: Target): string {
  if (t.kind === "work_order") return `work_order:${t.maintenance_ticket_id}`;
  if (t.kind === "project") return `project:${t.rehab_project_id}`;
  if (t.kind === "property") return `property:${t.property_id}`;
  return t.kind;
}

function targetFrom(v: string): Target | null {
  if (!v) return null;
  const [kind, id] = v.split(":");
  if (kind === "work_order")
    return { kind: "work_order", maintenance_ticket_id: id };
  if (kind === "project") return { kind: "project", rehab_project_id: id };
  if (kind === "property") return { kind: "property", property_id: id };
  return { kind: kind as EntryKind };
}

function EntryForm({
  entry,
  people,
  defaultUser,
  tickets,
  properties,
  onDone,
}: {
  entry: TimeEntry | null;
  people: Employee[];
  defaultUser: string;
  tickets: MaintenanceTicket[];
  properties: Property[];
  onDone: () => void;
}) {
  const [userId, setUserId] = useState(entry?.user_id ?? defaultUser);
  const [target, setTarget] = useState(entry ? targetValue(entry) : "");
  const [start, setStart] = useState(
    entry ? rfc3339ToLocal(entry.started_at) : ""
  );
  const [end, setEnd] = useState(
    entry?.ended_at ? rfc3339ToLocal(entry.ended_at) : ""
  );
  const [breakMin, setBreakMin] = useState(
    entry ? String(entry.break_minutes) : "0"
  );
  const [notes, setNotes] = useState(entry?.notes ?? "");
  const [busy, setBusy] = useState(false);

  const propName = useMemo(
    () => new Map(properties.map((p) => [p.id, p.name])),
    [properties]
  );
  const current = entry ? targetValue(entry) : "";
  const currentListed =
    !entry ||
    OTHER_KINDS.includes(entry.kind) ||
    (entry.kind === "work_order" &&
      tickets.some((t) => t.id === entry.maintenance_ticket_id)) ||
    (entry.kind === "property" &&
      properties.some((p) => p.id === entry.property_id));

  async function save() {
    const t = targetFrom(target);
    if (!entry && !userId) {
      toast.error("Pick who this time is for");
      return;
    }
    if (!t || !start) {
      toast.error("Pick the work and when it started");
      return;
    }
    const body: EntryInput = {
      ...t,
      started_at: localToRfc3339(start),
      ended_at: end ? localToRfc3339(end) : null,
      break_minutes: Number(breakMin) || 0,
      notes: notes.trim() || null,
    };
    if (!entry) body.user_id = userId;
    setBusy(true);
    try {
      if (entry) await team.editTime(entry.id, body);
      else await team.addTime(body);
      toast.success(entry ? "Time updated" : "Time added");
      onDone();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save that time"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>
          {entry ? `Fix ${entry.user_name}'s time` : "Add time for someone"}
        </DialogTitle>
        <DialogDescription>
          {entry?.approved
            ? "This time is already approved — saving a change keeps it on record."
            : "Use this for time someone forgot to clock."}
        </DialogDescription>
      </DialogHeader>
      <div className="space-y-4">
        {!entry && (
          <label className={label}>
            Person
            <select
              className={field}
              value={userId}
              onChange={(e) => setUserId(e.target.value)}
            >
              <option value="">Pick someone…</option>
              {people.map((p) => (
                <option key={p.user_id} value={p.user_id}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
        )}
        <label className={label}>
          What was it for?
          <select
            className={field}
            value={target}
            onChange={(e) => setTarget(e.target.value)}
          >
            <option value="">Pick…</option>
            {entry && !currentListed && (
              <option value={current}>{workLabel(entry)}</option>
            )}
            {tickets.length > 0 && (
              <optgroup label="Open work orders">
                {tickets.map((t) => (
                  <option key={t.id} value={`work_order:${t.id}`}>
                    {t.title}
                    {propName.get(t.property_id)
                      ? ` — ${propName.get(t.property_id)}`
                      : ""}
                  </option>
                ))}
              </optgroup>
            )}
            {properties.length > 0 && (
              <optgroup label="Properties">
                {properties.map((p) => (
                  <option key={p.id} value={`property:${p.id}`}>
                    {p.name}
                  </option>
                ))}
              </optgroup>
            )}
            <optgroup label="Other time">
              {OTHER_KINDS.map((k) => (
                <option key={k} value={k}>
                  {ENTRY_KIND_LABELS[k]}
                </option>
              ))}
            </optgroup>
          </select>
        </label>
        <div className="grid grid-cols-2 gap-3">
          <label className={label}>
            Started
            <input
              className={field}
              type="datetime-local"
              value={start}
              onChange={(e) => setStart(e.target.value)}
            />
          </label>
          <label className={label}>
            Finished
            <input
              className={field}
              type="datetime-local"
              value={end}
              min={start || undefined}
              onChange={(e) => setEnd(e.target.value)}
            />
          </label>
        </div>
        <label className={`${label} w-40`}>
          Break (minutes)
          <input
            className={field}
            type="number"
            min={0}
            value={breakMin}
            onChange={(e) => setBreakMin(e.target.value)}
          />
        </label>
        <label className={label}>
          Notes
          <textarea
            className={field}
            rows={2}
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        </label>
      </div>
      <DialogFooter>
        <Button onClick={save} disabled={busy}>
          {busy ? "Saving…" : entry ? "Save" : "Add time"}
        </Button>
      </DialogFooter>
    </>
  );
}

// ---- settle a missed punch --------------------------------------------------

function SettleForm({
  entry,
  onDone,
}: {
  entry: TimeEntry;
  onDone: () => void;
}) {
  const initial = entry.claimed_end ?? entry.ended_at;
  const [end, setEnd] = useState(initial ? rfc3339ToLocal(initial) : "");
  const [breakMin, setBreakMin] = useState(String(entry.break_minutes));
  const [busy, setBusy] = useState(false);

  async function save() {
    if (!end) {
      toast.error("Say when they finished");
      return;
    }
    setBusy(true);
    try {
      await team.resolve(entry.id, {
        ended_at: localToRfc3339(end),
        break_minutes: Number(breakMin) || 0,
      });
      toast.success("Missed punch settled");
      onDone();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't settle it"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>Settle missed punch</DialogTitle>
        <DialogDescription>
          {entry.user_name} started {fmtWhen(entry.started_at)} on{" "}
          {workLabel(entry)}.
        </DialogDescription>
      </DialogHeader>
      <div className="space-y-4">
        {(entry.missed_punch_reason ||
          entry.claimed_end ||
          entry.punch_note) && (
          <div className="rounded-xl bg-warn-soft/50 p-3 text-sm text-ink-2">
            {entry.missed_punch_reason && <p>{entry.missed_punch_reason}</p>}
            {entry.claimed_end && (
              <p>They say they finished {fmtWhen(entry.claimed_end)}.</p>
            )}
            {entry.punch_note && <p>“{entry.punch_note}”</p>}
          </div>
        )}
        <label className={label}>
          Finished at
          <input
            className={field}
            type="datetime-local"
            min={rfc3339ToLocal(entry.started_at)}
            value={end}
            onChange={(e) => setEnd(e.target.value)}
          />
        </label>
        <label className={`${label} w-40`}>
          Break (minutes)
          <input
            className={field}
            type="number"
            min={0}
            value={breakMin}
            onChange={(e) => setBreakMin(e.target.value)}
          />
        </label>
      </div>
      <DialogFooter>
        <Button onClick={save} disabled={busy}>
          {busy ? "Saving…" : "Settle"}
        </Button>
      </DialogFooter>
    </>
  );
}
