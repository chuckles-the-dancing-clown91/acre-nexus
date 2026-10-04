"use client";

// Timesheets: everyone's time for a week (or any dates), with approval,
// missed punches to settle, fixes, and PDF and CSV export. Reading needs
// `team:read`; changes need `team:manage`. Rates and labor cost only come
// back with `payroll:read`.

import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CalendarClock,
  Check,
  ChevronLeft,
  ChevronRight,
  Download,
  Lock,
  LogOut,
  Pencil,
  Plus,
  Trash2,
  Wrench,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useProperties } from "@/lib/queries";
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
  type TimeEntry,
} from "@/lib/backoffice";
import {
  OTHER_KINDS,
  addDays,
  distance,
  errMsg,
  fmtDay,
  fmtTime,
  fmtWhen,
  isOpenEntry,
  parseYmd,
  targetFrom,
  targetValue,
  useReady,
  workLabel,
} from "@/lib/money-extra";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { fieldClass, Label } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

type Status = "all" | "unapproved" | "missed" | "open" | "approved";
const STATUS_TABS = [
  ["all", "All"],
  ["unapproved", "Waiting approval"],
  ["missed", "Missed punches"],
  ["open", "On the clock"],
  ["approved", "Approved"],
] as const;

const CLOSED_TICKETS = new Set(["resolved", "closed", "cancelled"]);
const LOCKED = "Already billed to the owner, so this time is locked";
const check = "size-4 accent-[var(--accent)]";
const full = cn(fieldClass, "h-11 w-full");

export default function TimesheetsPage() {
  const { can } = useAuth();
  const ready = useReady("team:read");
  const allowed = useReady();
  const manage = can("team:manage");
  const qc = useQueryClient();

  const [from, setFrom] = useState(() => isoDate(weekStart(new Date())));
  const [to, setTo] = useState(() =>
    isoDate(addDays(weekStart(new Date()), 6))
  );
  const [custom, setCustom] = useState(false);
  const [userId, setUserId] = useState("");
  const [status, setStatus] = useState<Status>("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [editing, setEditing] = useState<TimeEntry | null>(null);
  const [adding, setAdding] = useState(false);
  const [settling, setSettling] = useState<TimeEntry | null>(null);
  const [busy, setBusy] = useState(false);

  const time = useQuery({
    queryKey: ["team-time", from, to, userId, status],
    queryFn: () =>
      team.time({
        from,
        to,
        user_id: userId || undefined,
        status: status === "all" ? undefined : status,
      }),
    enabled: ready,
  });
  const summary = useQuery({
    queryKey: ["team-time-summary", from, to],
    queryFn: () => team.summary({ from, to }),
    enabled: ready,
  });
  const roster = useQuery({
    queryKey: ["team-roster"],
    queryFn: team.roster,
    enabled: ready,
  });
  const tickets = useQuery({
    queryKey: ["tickets"],
    queryFn: () => api.tickets(),
    enabled: ready && manage,
  });
  const properties = useProperties({ enabled: ready && manage });

  const people = useMemo(
    () => (roster.data ?? []).filter((p) => p.profile),
    [roster.data]
  );
  const openTickets = useMemo(
    () => (tickets.data ?? []).filter((t) => !CLOSED_TICKETS.has(t.status)),
    [tickets.data]
  );
  const entries = useMemo(() => time.data ?? [], [time.data]);

  function reload() {
    setSelected(new Set());
    void qc.invalidateQueries({ queryKey: ["team-time"] });
    void qc.invalidateQueries({ queryKey: ["team-time-summary"] });
  }

  function shiftWeek(n: number) {
    const start = addDays(weekStart(parseYmd(from)), n * 7);
    setFrom(isoDate(start));
    setTo(isoDate(addDays(start, 6)));
    setCustom(false);
    setSelected(new Set());
  }

  function thisWeek() {
    const start = weekStart(new Date());
    setFrom(isoDate(start));
    setTo(isoDate(addDays(start, 6)));
    setCustom(false);
    setSelected(new Set());
  }

  async function run(fn: () => Promise<unknown>, ok: string, fail: string) {
    setBusy(true);
    try {
      await fn();
      toast.success(ok);
      reload();
    } catch (e) {
      toast.error(errMsg(e, fail));
    } finally {
      setBusy(false);
    }
  }

  const selectable = entries.filter((e) => !e.approved && !isOpenEntry(e));
  const picked = selectable.filter((e) => selected.has(e.id));
  const allSelected =
    selectable.length > 0 && selectable.every((e) => selected.has(e.id));

  async function approveSelected() {
    const ids = picked.map((e) => e.id);
    if (ids.length === 0) return;
    setBusy(true);
    try {
      const res = await team.approveMany(ids);
      if (res.skipped.length === 0) {
        toast.success(
          `Approved ${res.approved} ${res.approved === 1 ? "entry" : "entries"}`
        );
      } else {
        const reasons = [...new Set(res.skipped.map((s) => s.reason))];
        toast.warning(
          `Approved ${res.approved}, skipped ${res.skipped.length}`,
          { description: reasons.join(" · ") }
        );
      }
      reload();
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

  if (allowed && !ready)
    return (
      <div className="space-y-6">
        <PageHeader eyebrow="Team" title="Timesheets" />
        <Panel>
          <EmptyState
            icon={<Lock />}
            title="You don't have access to timesheets"
            description="Ask an admin for the team:read permission."
          />
        </Panel>
      </div>
    );

  const totalMinutes = entries.reduce((s, e) => s + e.minutes, 0);
  const seeCost = entries.some((e) => e.labor_cost_cents != null);
  const summaryRows = userId
    ? (summary.data ?? []).filter((s) => s.user_id === userId)
    : (summary.data ?? []);
  const colCount = 8 + (manage ? 2 : 0) + (seeCost ? 1 : 0);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Team"
        title="Timesheets"
        description="Check and approve everyone's hours before payroll and owner billing."
        actions={
          <>
            <Button variant="secondary" onClick={() => exportAs("pdf")}>
              <Download />
              PDF
            </Button>
            <Button variant="secondary" onClick={() => exportAs("csv")}>
              <Download />
              CSV
            </Button>
            {manage && (
              <Button onClick={() => setAdding(true)}>
                <Plus />
                Add time
              </Button>
            )}
          </>
        }
      />

      <Panel className="flex flex-wrap items-end gap-3 p-4">
        {custom ? (
          <>
            <div className="space-y-1.5">
              <Label htmlFor="ts-from">From</Label>
              <input
                id="ts-from"
                className={cn(fieldClass, "block")}
                type="date"
                value={from}
                onChange={(e) => e.target.value && setFrom(e.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="ts-to">To</Label>
              <input
                id="ts-to"
                className={cn(fieldClass, "block")}
                type="date"
                value={to}
                min={from}
                onChange={(e) => e.target.value && setTo(e.target.value)}
              />
            </div>
            <Button variant="ghost" onClick={thisWeek}>
              Back to weeks
            </Button>
          </>
        ) : (
          <div className="flex flex-wrap items-center gap-2">
            <Button
              size="icon"
              variant="secondary"
              aria-label="Previous week"
              onClick={() => shiftWeek(-1)}
            >
              <ChevronLeft />
            </Button>
            <div className="min-w-40 text-center">
              <div className="eyebrow">Week of</div>
              <div className="text-[14px] font-medium text-fg">
                {parseYmd(from).toLocaleDateString([], {
                  month: "short",
                  day: "numeric",
                })}{" "}
                to{" "}
                {parseYmd(to).toLocaleDateString([], {
                  month: "short",
                  day: "numeric",
                  year: "numeric",
                })}
              </div>
            </div>
            <Button
              size="icon"
              variant="secondary"
              aria-label="Next week"
              onClick={() => shiftWeek(1)}
            >
              <ChevronRight />
            </Button>
            <Button size="sm" variant="ghost" onClick={thisWeek}>
              This week
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setCustom(true)}>
              <CalendarClock />
              Pick dates
            </Button>
          </div>
        )}
        <div className="ml-auto w-full space-y-1.5 sm:w-56">
          <Label htmlFor="ts-person">Person</Label>
          <select
            id="ts-person"
            className={cn(fieldClass, "block w-full")}
            value={userId}
            onChange={(e) => {
              setUserId(e.target.value);
              setSelected(new Set());
            }}
          >
            <option value="">Everyone</option>
            {people.map((p) => (
              <option key={p.user_id} value={p.user_id}>
                {p.name}
              </option>
            ))}
          </select>
        </div>
      </Panel>

      {summaryRows.length > 0 && (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {summaryRows.map((s) => {
            const on = userId === s.user_id;
            return (
              <button
                key={s.user_id}
                type="button"
                aria-pressed={on}
                onClick={() => setUserId(on ? "" : s.user_id)}
                className={cn(
                  "glass rounded-2xl p-4 text-left transition hover:-translate-y-px hover:border-line-strong",
                  on && "border-accent"
                )}
              >
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate text-[14px] font-medium text-fg">
                    {s.name}
                  </span>
                  {s.clocked_in && (
                    <Badge tone="good" dot>
                      on
                    </Badge>
                  )}
                </div>
                <div className="figure mt-1.5 text-[24px] leading-none font-semibold text-fg">
                  {hm(s.minutes)}
                </div>
                <div className="mt-1.5 space-y-0.5 text-xs text-fg-3">
                  <div>
                    {hm(s.on_work_minutes)} on work · {hm(s.other_minutes)}{" "}
                    other
                  </div>
                  {(s.unapproved_minutes > 0 || s.missed_punches > 0) && (
                    <div className="text-warn">
                      {[
                        s.unapproved_minutes > 0 &&
                          `${hm(s.unapproved_minutes)} waiting approval`,
                        s.missed_punches > 0 &&
                          `${s.missed_punches} missed ${s.missed_punches === 1 ? "punch" : "punches"}`,
                      ]
                        .filter(Boolean)
                        .join(" · ")}
                    </div>
                  )}
                  {s.labor_cost_cents != null && (
                    <div>Labor cost {money(s.labor_cost_cents)}</div>
                  )}
                </div>
              </button>
            );
          })}
        </div>
      )}

      <div className="flex flex-wrap items-center justify-between gap-3">
        <Tabs
          tabs={STATUS_TABS}
          value={status}
          onChange={(s) => {
            setStatus(s);
            setSelected(new Set());
          }}
        />
        {manage && (
          <Button
            onClick={approveSelected}
            disabled={busy || picked.length === 0}
          >
            <Check />
            Approve selected{picked.length > 0 && ` (${picked.length})`}
          </Button>
        )}
      </div>

      {time.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          <p role="alert">{time.error.message}</p>
        </Panel>
      )}

      <Panel className="overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[1000px] text-[13px]">
            <thead>
              <tr className="border-b border-line text-left">
                {manage && (
                  <th scope="col" className="w-10 px-4 py-2.5">
                    <input
                      type="checkbox"
                      className={check}
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
                {["Date", "Person", "Work", "In", "Out"].map((h) => (
                  <th
                    key={h}
                    scope="col"
                    className="eyebrow px-3 py-2.5 font-medium"
                  >
                    {h}
                  </th>
                ))}
                <th
                  scope="col"
                  className="eyebrow px-3 py-2.5 text-right font-medium"
                >
                  Break
                </th>
                <th
                  scope="col"
                  className="eyebrow px-3 py-2.5 text-right font-medium"
                >
                  Hours
                </th>
                {seeCost && (
                  <th
                    scope="col"
                    className="eyebrow px-3 py-2.5 text-right font-medium"
                  >
                    Cost
                  </th>
                )}
                <th scope="col" className="eyebrow px-3 py-2.5 font-medium">
                  Status
                </th>
                {manage && (
                  <th scope="col" className="px-4 py-2.5">
                    <span className="sr-only">Actions</span>
                  </th>
                )}
              </tr>
            </thead>
            <tbody>
              {time.isLoading &&
                Array.from({ length: 5 }, (_, i) => (
                  <tr key={i}>
                    <td colSpan={colCount} className="px-4 py-2">
                      <Skeleton className="h-9" />
                    </td>
                  </tr>
                ))}
              {time.data && entries.length === 0 && (
                <tr>
                  <td
                    colSpan={colCount}
                    className="px-4 py-10 text-center text-fg-3"
                  >
                    No time here.
                  </td>
                </tr>
              )}
              {entries.map((e) => {
                const open = isOpenEntry(e);
                return (
                  <tr
                    key={e.id}
                    className={cn(
                      "border-b border-line/60 align-top last:border-0 hover:bg-surface-2/60",
                      selected.has(e.id) && "bg-accent/5"
                    )}
                  >
                    {manage && (
                      <td className="px-4 py-2.5">
                        {!e.approved && !open && (
                          <input
                            type="checkbox"
                            className={check}
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
                    <td className="px-3 py-2.5 whitespace-nowrap text-fg-2">
                      {fmtDay(e.started_at)}
                    </td>
                    <td className="px-3 py-2.5 font-medium text-fg">
                      {e.user_name}
                    </td>
                    <td className="px-3 py-2.5">
                      <div className="text-fg">{workLabel(e)}</div>
                      <div className="text-xs text-fg-3">
                        {[
                          // The kind, unless the title already says it.
                          workLabel(e) !== ENTRY_KIND_LABELS[e.kind]
                            ? ENTRY_KIND_LABELS[e.kind]
                            : null,
                          e.kind !== "property" ? e.property_name : null,
                          e.notes,
                        ]
                          .filter(Boolean)
                          .join(" · ")}
                      </div>
                    </td>
                    <td className="figure px-3 py-2.5 whitespace-nowrap text-fg-2">
                      {fmtTime(e.started_at)}
                    </td>
                    <td className="figure px-3 py-2.5 whitespace-nowrap text-fg-2">
                      {open ? (
                        <span className="text-good">still on</span>
                      ) : (
                        fmtTime(e.ended_at)
                      )}
                    </td>
                    <td className="figure px-3 py-2.5 text-right text-fg-2">
                      {e.break_minutes ? `${e.break_minutes}m` : "—"}
                    </td>
                    <td className="figure px-3 py-2.5 text-right font-semibold text-fg">
                      {hm(e.minutes)}
                    </td>
                    {seeCost && (
                      <td
                        className="figure px-3 py-2.5 text-right text-fg-2"
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
                    <td className="px-3 py-2.5">
                      <div className="flex flex-wrap gap-1">
                        {open && (
                          <Badge tone="accent" dot>
                            on the clock
                          </Badge>
                        )}
                        {e.approved && <Badge tone="good">approved</Badge>}
                        {e.billed && <Badge tone="info">billed, locked</Badge>}
                        {(e.missed_punch || e.needs_review) && (
                          <span
                            title={[
                              e.missed_punch_reason,
                              e.claimed_end &&
                                `They say they finished ${fmtWhen(e.claimed_end)}`,
                              e.punch_note && `"${e.punch_note}"`,
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
                        {!e.approved && !open && !e.needs_review && (
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
                        <div className="mt-1 text-xs text-fg-3">
                          {e.missed_punch_reason}
                          {e.claimed_end && (
                            <> · says done {fmtWhen(e.claimed_end)}</>
                          )}
                          {e.punch_note && <> · &ldquo;{e.punch_note}&rdquo;</>}
                        </div>
                      )}
                    </td>
                    {manage && (
                      <td className="px-4 py-2 text-right whitespace-nowrap">
                        <div className="flex justify-end gap-1">
                          {e.needs_review && (
                            <Button
                              size="sm"
                              variant="secondary"
                              onClick={() => setSettling(e)}
                            >
                              <Wrench />
                              Settle
                            </Button>
                          )}
                          {open && (
                            <Button
                              size="sm"
                              variant="secondary"
                              disabled={busy}
                              onClick={() =>
                                run(
                                  () => team.clockOut(e.id),
                                  `Clocked ${e.user_name} out`,
                                  "Couldn't clock them out"
                                )
                              }
                            >
                              <LogOut />
                              Clock out
                            </Button>
                          )}
                          {!e.approved && !open && !e.needs_review && (
                            <Button
                              size="sm"
                              variant="secondary"
                              disabled={busy}
                              onClick={() =>
                                run(
                                  () => team.approve(e.id),
                                  "Approved",
                                  "Couldn't approve that"
                                )
                              }
                            >
                              <Check />
                              Approve
                            </Button>
                          )}
                          <Button
                            size="icon"
                            variant="ghost"
                            aria-label="Edit"
                            disabled={e.billed}
                            title={e.billed ? LOCKED : "Edit"}
                            onClick={() => setEditing(e)}
                          >
                            <Pencil />
                          </Button>
                          <Button
                            size="icon"
                            variant="ghost"
                            aria-label="Delete"
                            className="hover:text-bad"
                            disabled={busy || e.billed}
                            title={e.billed ? LOCKED : "Delete"}
                            onClick={() => {
                              if (
                                !window.confirm(
                                  `Delete this time for ${e.user_name}?`
                                )
                              )
                                return;
                              void run(
                                () => team.deleteTime(e.id),
                                "Time deleted",
                                "Couldn't delete that"
                              );
                            }}
                          >
                            <Trash2 />
                          </Button>
                        </div>
                      </td>
                    )}
                  </tr>
                );
              })}
            </tbody>
            {entries.length > 0 && (
              <tfoot>
                <tr className="border-t-2 border-line font-semibold text-fg">
                  <td
                    colSpan={manage ? 7 : 6}
                    className="px-3 py-2.5 text-right text-fg-3"
                  >
                    {entries.length}{" "}
                    {entries.length === 1 ? "entry" : "entries"}
                  </td>
                  <td className="figure px-3 py-2.5 text-right">
                    {hm(totalMinutes)}
                  </td>
                  <td colSpan={colCount - (manage ? 8 : 7)} />
                </tr>
              </tfoot>
            )}
          </table>
        </div>
      </Panel>

      {(adding || editing) && (
        <EntryDialog
          key={editing?.id ?? "new"}
          entry={editing}
          people={people}
          defaultUser={userId}
          tickets={openTickets}
          properties={properties.data ?? []}
          onClose={() => {
            setAdding(false);
            setEditing(null);
          }}
          onDone={() => {
            setAdding(false);
            setEditing(null);
            reload();
          }}
        />
      )}

      {settling && (
        <SettleDialog
          key={settling.id}
          entry={settling}
          onClose={() => setSettling(null)}
          onDone={() => {
            setSettling(null);
            reload();
          }}
        />
      )}
    </div>
  );
}

// ---- add / edit -------------------------------------------------------------

function EntryDialog({
  entry,
  people,
  defaultUser,
  tickets,
  properties,
  onClose,
  onDone,
}: {
  entry: TimeEntry | null;
  people: Employee[];
  defaultUser: string;
  tickets: MaintenanceTicket[];
  properties: Property[];
  onClose: () => void;
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
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto">
        <DialogTitle className="text-[17px] font-semibold">
          {entry ? `Fix ${entry.user_name}'s time` : "Add time for someone"}
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          {entry?.approved
            ? "This time is already approved. A change you save stays on record."
            : "For time someone forgot to clock."}
        </DialogDescription>
        <div className="mt-5 space-y-4">
          {!entry && (
            <Labeled label="Person">
              <select
                className={full}
                value={userId}
                onChange={(e) => setUserId(e.target.value)}
              >
                <option value="">Pick someone</option>
                {people.map((p) => (
                  <option key={p.user_id} value={p.user_id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </Labeled>
          )}
          <Labeled label="What was it for?">
            <select
              className={full}
              value={target}
              onChange={(e) => setTarget(e.target.value)}
            >
              <option value="">Pick</option>
              {entry && !currentListed && (
                <option value={current}>{workLabel(entry)}</option>
              )}
              {tickets.length > 0 && (
                <optgroup label="Open work orders">
                  {tickets.map((t) => (
                    <option key={t.id} value={`work_order:${t.id}`}>
                      {t.title}
                      {propName.get(t.property_id)
                        ? ` · ${propName.get(t.property_id)}`
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
          </Labeled>
          <div className="grid gap-3 sm:grid-cols-2">
            <Labeled label="Started">
              <input
                className={full}
                type="datetime-local"
                value={start}
                onChange={(e) => setStart(e.target.value)}
              />
            </Labeled>
            <Labeled label="Finished">
              <input
                className={full}
                type="datetime-local"
                value={end}
                min={start || undefined}
                onChange={(e) => setEnd(e.target.value)}
              />
            </Labeled>
          </div>
          <Labeled label="Break (minutes)" className="w-40">
            <input
              className={full}
              type="number"
              min={0}
              inputMode="numeric"
              value={breakMin}
              onChange={(e) => setBreakMin(e.target.value)}
            />
          </Labeled>
          <Labeled label="Notes">
            <textarea
              className={cn(fieldClass, "min-h-[64px] w-full")}
              rows={2}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
            />
          </Labeled>
        </div>
        <div className="mt-6 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={save} loading={busy}>
            {entry ? "Save" : "Add time"}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// ---- settle a missed punch --------------------------------------------------

function SettleDialog({
  entry,
  onClose,
  onDone,
}: {
  entry: TimeEntry;
  onClose: () => void;
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
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          Settle missed punch
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          {entry.user_name} started {fmtWhen(entry.started_at)} on{" "}
          {workLabel(entry)}.
        </DialogDescription>
        <div className="mt-5 space-y-4">
          {(entry.missed_punch_reason ||
            entry.claimed_end ||
            entry.punch_note) && (
            <div className="space-y-1 rounded-xl border border-warn/30 bg-warn/10 p-3 text-[13px] text-fg-2">
              {entry.missed_punch_reason && <p>{entry.missed_punch_reason}</p>}
              {entry.claimed_end && (
                <p>They say they finished {fmtWhen(entry.claimed_end)}.</p>
              )}
              {entry.punch_note && <p>&ldquo;{entry.punch_note}&rdquo;</p>}
            </div>
          )}
          <Labeled label="Finished at">
            <input
              className={full}
              type="datetime-local"
              min={rfc3339ToLocal(entry.started_at)}
              value={end}
              onChange={(e) => setEnd(e.target.value)}
            />
          </Labeled>
          <Labeled label="Break (minutes)" className="w-40">
            <input
              className={full}
              type="number"
              min={0}
              inputMode="numeric"
              value={breakMin}
              onChange={(e) => setBreakMin(e.target.value)}
            />
          </Labeled>
        </div>
        <div className="mt-6 flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={save} loading={busy}>
            <Check />
            Settle
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function Labeled({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <label className={cn("block space-y-1.5", className)}>
      <span className="text-[13px] font-medium text-fg-2">{label}</span>
      {children}
    </label>
  );
}
