"use client";

// My time: self-service for anyone with an employee profile — the clock,
// missed punches, this week's entries, weekly totals, time off, upcoming
// shifts, and mileage / receipts.

import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { ApiError } from "@/lib/api";
import {
  ENTRY_KIND_LABELS,
  EXPENSE_CATEGORIES,
  currentLocation,
  expenses,
  hm,
  isoDate,
  localToRfc3339,
  me,
  money,
  rfc3339ToLocal,
  toCents,
  weekStart,
  type ClockState,
  type EntryInput,
  type EntryKind,
  type Expense,
  type ExpenseCategory,
  type ExpenseInput,
  type Shift,
  type Target,
  type TimeEntry,
  type TimeOff,
  type WeekHours,
  type WorkOption,
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

function fmtDay(iso: string) {
  return new Date(iso).toLocaleDateString([], {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
}

function fmtDate(ymd: string) {
  const [y, m, d] = ymd.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString([], {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function fmtTime(iso: string | null) {
  if (!iso) return "—";
  return new Date(iso).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

/** What an entry was for, in plain words. */
function workLabel(e: TimeEntry) {
  if (e.kind === "work_order")
    return e.work_order_title ?? ENTRY_KIND_LABELS.work_order;
  if (e.kind === "project") return e.project_name ?? ENTRY_KIND_LABELS.project;
  if (e.kind === "property")
    return e.property_name ?? ENTRY_KIND_LABELS.property;
  return ENTRY_KIND_LABELS[e.kind];
}

// ---- work picker ------------------------------------------------------------

const OTHER_KINDS: EntryKind[] = ["travel", "shop", "admin", "other"];

/** Encode a target as a single select value. */
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

function optionText(o: WorkOption) {
  return o.property_name && o.kind !== "property"
    ? `${o.label} — ${o.property_name}`
    : o.label;
}

function WorkSelect({
  work,
  value,
  onChange,
  includeOther = true,
  only,
  placeholder = "Pick what you're working on…",
}: {
  work: WorkOption[];
  value: string;
  onChange: (v: string) => void;
  includeOther?: boolean;
  only?: WorkOption["kind"];
  placeholder?: string;
}) {
  const list = only ? work.filter((w) => w.kind === only) : work;
  const mine = list.filter((w) => w.mine);
  const groups: [string, WorkOption[]][] = [
    ["Assigned to you", mine],
    ["Work orders", list.filter((w) => !w.mine && w.kind === "work_order")],
    ["Rehab projects", list.filter((w) => !w.mine && w.kind === "project")],
    ["Properties", list.filter((w) => !w.mine && w.kind === "property")],
  ];
  return (
    <select
      className={field}
      value={value}
      onChange={(e) => onChange(e.target.value)}
    >
      <option value="">{placeholder}</option>
      {groups
        .filter(([, items]) => items.length > 0)
        .map(([name, items]) => (
          <optgroup key={name} label={name}>
            {items.map((o) => (
              <option key={`${o.kind}:${o.id}`} value={`${o.kind}:${o.id}`}>
                {optionText(o)}
              </option>
            ))}
          </optgroup>
        ))}
      {includeOther && (
        <optgroup label="Other time">
          {OTHER_KINDS.map((k) => (
            <option key={k} value={k}>
              {ENTRY_KIND_LABELS[k]}
            </option>
          ))}
        </optgroup>
      )}
    </select>
  );
}

// ---- page -------------------------------------------------------------------

export default function MyTimePage() {
  const [clock, setClock] = useState<ClockState | null>(null);
  const [notOnTeam, setNotOnTeam] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [work, setWork] = useState<WorkOption[]>([]);
  const [week, setWeek] = useState(() => weekStart(new Date()));
  const [entries, setEntries] = useState<TimeEntry[]>([]);
  const [recent, setRecent] = useState<TimeEntry[]>([]);
  const [hours, setHours] = useState<WeekHours[]>([]);

  const loadClock = useCallback(() => {
    me.clock()
      .then(setClock)
      .catch((e) => {
        if (e instanceof ApiError && e.status === 403) setNotOnTeam(true);
        else setError(errMsg(e));
      });
  }, []);

  const loadWeek = useCallback(() => {
    me.time({ from: isoDate(week), to: isoDate(addDays(week, 6)) })
      .then(setEntries)
      .catch(() => undefined);
  }, [week]);

  const loadRecent = useCallback(() => {
    const today = new Date();
    me.time({ from: isoDate(addDays(today, -90)), to: isoDate(today) })
      .then(setRecent)
      .catch(() => undefined);
    const thisWeek = weekStart(today);
    me.hours({
      from: isoDate(addDays(thisWeek, -7 * 7)),
      to: isoDate(addDays(thisWeek, 6)),
    })
      .then(setHours)
      .catch(() => undefined);
  }, []);

  const refreshAll = useCallback(() => {
    loadClock();
    loadWeek();
    loadRecent();
  }, [loadClock, loadWeek, loadRecent]);

  useEffect(() => {
    loadClock();
    loadRecent();
    me.work()
      .then(setWork)
      .catch(() => undefined);
  }, [loadClock, loadRecent]);

  useEffect(() => {
    loadWeek();
  }, [loadWeek]);

  if (notOnTeam) {
    return (
      <div className="space-y-6">
        <Header />
        <Card className="p-8 text-center">
          <p className="font-display text-xl font-bold">
            You&apos;re not on the team yet
          </p>
          <p className="mt-2 text-ink-3">
            Ask the office to add your employee profile, then come back here to
            clock in.
          </p>
        </Card>
      </div>
    );
  }

  const missed = recent.filter((e) => e.needs_review && !e.claimed_end);

  return (
    <div className="space-y-6">
      <Header />
      {error && (
        <Card className="p-4 text-sm text-bad">
          <p role="alert">{error}</p>
        </Card>
      )}

      {clock ? (
        <ClockCard clock={clock} work={work} onChanged={refreshAll} />
      ) : (
        !error && <Card className="p-10 text-center text-ink-3">Loading…</Card>
      )}

      {clock && clock.missed_punches > 0 && missed.length > 0 && (
        <MissedPunches entries={missed} onChanged={refreshAll} />
      )}

      <WeekEntries
        week={week}
        setWeek={setWeek}
        entries={entries}
        work={work}
        onChanged={refreshAll}
      />

      <MyWeeks hours={hours} />

      <div className="grid gap-6 lg:grid-cols-2">
        <TimeOffSection />
        <UpcomingShifts />
      </div>

      <ExpensesSection work={work} />
    </div>
  );
}

function Header() {
  return (
    <div>
      <h1 className="font-display text-3xl font-extrabold tracking-tight">
        My time
      </h1>
      <p className="text-ink-3">
        Clock in and out, fix a missed punch, ask for time off, and log miles
        and receipts.
      </p>
    </div>
  );
}

// ---- clock ------------------------------------------------------------------

function Elapsed({ since }: { since: string }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  const secs = Math.max(
    0,
    Math.floor((now - new Date(since).getTime()) / 1000)
  );
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  return (
    <span className="font-display text-5xl font-extrabold tabular-nums tracking-tight">
      {h}:{String(m).padStart(2, "0")}:{String(s).padStart(2, "0")}
    </span>
  );
}

function ClockCard({
  clock,
  work,
  onChanged,
}: {
  clock: ClockState;
  work: WorkOption[];
  onChanged: () => void;
}) {
  const [target, setTarget] = useState("");
  const [note, setNote] = useState("");
  const [breakMin, setBreakMin] = useState("");
  const [busy, setBusy] = useState(false);
  const open = clock.open;

  async function where() {
    return clock.records_location
      ? ((await currentLocation()) ?? undefined)
      : undefined;
  }

  async function clockIn() {
    const t = targetFrom(target);
    if (!t) {
      toast.error("Pick what you're working on first");
      return;
    }
    setBusy(true);
    try {
      const location = await where();
      await me.clockIn({ ...t, notes: note.trim() || undefined, location });
      toast.success("You're clocked in");
      setNote("");
      setTarget("");
      onChanged();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't clock you in"));
    } finally {
      setBusy(false);
    }
  }

  async function clockOut() {
    setBusy(true);
    try {
      const location = await where();
      const b = Number(breakMin);
      await me.clockOut({
        location,
        break_minutes: breakMin && Number.isFinite(b) ? b : undefined,
        notes: note.trim() || undefined,
      });
      toast.success("You're clocked out");
      setNote("");
      setBreakMin("");
      onChanged();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't clock you out"));
    } finally {
      setBusy(false);
    }
  }

  const totals = (
    <div className="flex gap-6 text-sm">
      <div>
        <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
          Today
        </div>
        <div className="font-display text-xl font-bold">
          {hm(clock.today_minutes)}
        </div>
      </div>
      <div>
        <div className="text-xs font-semibold uppercase tracking-wide text-ink-3">
          This week
        </div>
        <div className="font-display text-xl font-bold">
          {hm(clock.week_minutes)}
        </div>
      </div>
    </div>
  );

  return (
    <Card className={`p-6 ${open ? "border-good" : ""}`}>
      {open ? (
        <div className="space-y-5">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div className="space-y-2">
              <Badge tone="good">On the clock</Badge>
              <div className="font-display text-2xl font-bold">
                {workLabel(open)}
              </div>
              <div className="text-sm text-ink-3">
                {ENTRY_KIND_LABELS[open.kind]}
                {open.property_name && open.kind !== "property" && (
                  <> · {open.property_name}</>
                )}{" "}
                · since {fmtTime(open.started_at)}
              </div>
            </div>
            <Elapsed since={open.started_at} />
          </div>
          {totals}
          <div className="flex flex-wrap items-end gap-3 border-t border-line pt-5">
            <label className={`${label} w-36`}>
              Break (minutes)
              <input
                className={field}
                type="number"
                min={0}
                inputMode="numeric"
                placeholder="0"
                value={breakMin}
                onChange={(e) => setBreakMin(e.target.value)}
              />
            </label>
            <label className={`${label} min-w-48 flex-1`}>
              Note (optional)
              <input
                className={field}
                placeholder="What got done"
                value={note}
                onChange={(e) => setNote(e.target.value)}
              />
            </label>
            <Button onClick={clockOut} disabled={busy} className="px-6">
              {busy ? "Clocking out…" : "Clock out"}
            </Button>
          </div>
        </div>
      ) : (
        <div className="space-y-5">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div>
              <Badge>Not clocked in</Badge>
              <div className="mt-2 font-display text-2xl font-bold">
                What are you working on?
              </div>
            </div>
            {totals}
          </div>
          <div className="space-y-3">
            <WorkSelect
              work={work}
              value={target}
              onChange={setTarget}
              includeOther={false}
            />
            <div className="flex flex-wrap gap-2">
              {(["travel", "shop", "admin"] as const).map((k) => (
                <button
                  key={k}
                  onClick={() => setTarget(k)}
                  className={`rounded-lg border px-3 py-1.5 text-sm font-semibold ${
                    target === k
                      ? "border-accent bg-accent-soft text-accent-2"
                      : "border-line text-ink-2 hover:bg-surface-2"
                  }`}
                >
                  {k === "travel"
                    ? "Travel"
                    : k === "shop"
                      ? "Shop time"
                      : "Office time"}
                </button>
              ))}
            </div>
            <div className="flex flex-wrap items-end gap-3">
              <label className={`${label} min-w-48 flex-1`}>
                Note (optional)
                <input
                  className={field}
                  placeholder="Anything the office should know"
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                />
              </label>
              <Button
                onClick={clockIn}
                disabled={busy || !target}
                className="px-6"
              >
                {busy ? "Clocking in…" : "Clock in"}
              </Button>
            </div>
          </div>
        </div>
      )}
      <div className="mt-5 space-y-1 text-xs text-ink-3">
        {clock.records_location && (
          <p>We note where you are only when you clock in or out.</p>
        )}
        {clock.overtime_rule && <p>{clock.overtime_rule}</p>}
      </div>
    </Card>
  );
}

// ---- missed punches ---------------------------------------------------------

function MissedPunches({
  entries,
  onChanged,
}: {
  entries: TimeEntry[];
  onChanged: () => void;
}) {
  return (
    <Card className="border-warn bg-warn-soft/40 p-5">
      <h2 className="font-display text-lg font-bold text-warn">
        You forgot to clock out
      </h2>
      <p className="mb-4 text-sm text-ink-2">
        Tell us when you finished and the office will settle it.
      </p>
      <div className="space-y-3">
        {entries.map((e) => (
          <MissedRow key={e.id} entry={e} onChanged={onChanged} />
        ))}
      </div>
    </Card>
  );
}

function MissedRow({
  entry,
  onChanged,
}: {
  entry: TimeEntry;
  onChanged: () => void;
}) {
  const [end, setEnd] = useState("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);

  async function save() {
    if (!end) {
      toast.error("Say when you finished");
      return;
    }
    setBusy(true);
    try {
      await me.claim(entry.id, localToRfc3339(end), note.trim() || undefined);
      toast.success("Thanks — the office will take a look");
      onChanged();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save that"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-wrap items-end gap-3 rounded-xl bg-surface p-3">
      <div className="min-w-48 flex-1">
        <div className="font-semibold">{workLabel(entry)}</div>
        <div className="text-sm text-ink-3">
          Started {fmtDay(entry.started_at)} at {fmtTime(entry.started_at)}
          {entry.missed_punch_reason && <> · {entry.missed_punch_reason}</>}
        </div>
      </div>
      <label className={label}>
        When did you finish?
        <input
          className={field}
          type="datetime-local"
          min={rfc3339ToLocal(entry.started_at)}
          value={end}
          onChange={(e) => setEnd(e.target.value)}
        />
      </label>
      <label className={`${label} min-w-40`}>
        Note
        <input
          className={field}
          placeholder="Optional"
          value={note}
          onChange={(e) => setNote(e.target.value)}
        />
      </label>
      <Button onClick={save} disabled={busy}>
        {busy ? "Saving…" : "Send"}
      </Button>
    </div>
  );
}

// ---- this week --------------------------------------------------------------

function entryStatus(e: TimeEntry) {
  if (!e.ended_at && !e.needs_review)
    return <Badge tone="accent">on the clock</Badge>;
  if (e.billed) return <Badge tone="info">billed</Badge>;
  if (e.approved) return <Badge tone="good">approved</Badge>;
  if (e.missed_punch || e.needs_review)
    return <Badge tone="warn">missed punch</Badge>;
  return <Badge>waiting</Badge>;
}

function canChange(e: TimeEntry) {
  return (
    !!e.ended_at &&
    !e.approved &&
    !e.billed &&
    !e.missed_punch &&
    !e.needs_review
  );
}

function WeekEntries({
  week,
  setWeek,
  entries,
  work,
  onChanged,
}: {
  week: Date;
  setWeek: (d: Date) => void;
  entries: TimeEntry[];
  work: WorkOption[];
  onChanged: () => void;
}) {
  const [editing, setEditing] = useState<TimeEntry | null>(null);
  const [adding, setAdding] = useState(false);
  const total = entries.reduce((s, e) => s + e.minutes, 0);
  const end = addDays(week, 6);

  return (
    <Card className="overflow-hidden">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-4">
        <div>
          <h2 className="font-display text-lg font-bold">This week</h2>
          <p className="text-sm text-ink-3">
            {week.toLocaleDateString([], { month: "short", day: "numeric" })} –{" "}
            {end.toLocaleDateString([], { month: "short", day: "numeric" })} ·{" "}
            {hm(total)} hours
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={() => setWeek(addDays(week, -7))}>
            ← Prev
          </Button>
          <Button
            variant="ghost"
            onClick={() => setWeek(weekStart(new Date()))}
          >
            This week
          </Button>
          <Button variant="outline" onClick={() => setWeek(addDays(week, 7))}>
            Next →
          </Button>
          <Button onClick={() => setAdding(true)}>Add time I forgot</Button>
        </div>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="text-left text-xs font-bold uppercase tracking-wide text-ink-3">
            <tr className="border-b border-line">
              <th className="px-5 py-3">Date</th>
              <th className="px-3 py-3">Work</th>
              <th className="px-3 py-3">In – out</th>
              <th className="px-3 py-3 text-right">Break</th>
              <th className="px-3 py-3 text-right">Hours</th>
              <th className="px-3 py-3">Status</th>
              <th className="px-5 py-3" />
            </tr>
          </thead>
          <tbody className="divide-y divide-line">
            {entries.map((e) => (
              <tr key={e.id}>
                <td className="whitespace-nowrap px-5 py-3">
                  {fmtDay(e.started_at)}
                </td>
                <td className="px-3 py-3">
                  <div className="font-semibold">{workLabel(e)}</div>
                  {e.notes && (
                    <div className="text-xs text-ink-3">{e.notes}</div>
                  )}
                </td>
                <td className="whitespace-nowrap px-3 py-3">
                  {fmtTime(e.started_at)} – {fmtTime(e.ended_at)}
                </td>
                <td className="px-3 py-3 text-right">
                  {e.break_minutes ? `${e.break_minutes}m` : "—"}
                </td>
                <td className="px-3 py-3 text-right font-semibold tabular-nums">
                  {hm(e.minutes)}
                </td>
                <td className="px-3 py-3">{entryStatus(e)}</td>
                <td className="px-5 py-3 text-right">
                  {canChange(e) && (
                    <button
                      onClick={() => setEditing(e)}
                      className="rounded-lg border border-line px-2 py-1 text-xs font-semibold text-ink-2 hover:border-accent"
                    >
                      Edit
                    </button>
                  )}
                </td>
              </tr>
            ))}
            {entries.length === 0 && (
              <tr>
                <td colSpan={7} className="px-5 py-10 text-center text-ink-3">
                  No time this week.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <Dialog
        open={adding || !!editing}
        onOpenChange={(o) => {
          if (!o) {
            setAdding(false);
            setEditing(null);
          }
        }}
      >
        <DialogContent>
          {(adding || editing) && (
            <EntryForm
              key={editing?.id ?? "new"}
              entry={editing}
              work={work}
              onDone={() => {
                setAdding(false);
                setEditing(null);
                onChanged();
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </Card>
  );
}

function EntryForm({
  entry,
  work,
  onDone,
}: {
  entry: TimeEntry | null;
  work: WorkOption[];
  onDone: () => void;
}) {
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

  async function save() {
    const t = targetFrom(target);
    if (!t || !start || !end) {
      toast.error("Pick the work and fill in when you started and finished");
      return;
    }
    const body: EntryInput = {
      ...t,
      started_at: localToRfc3339(start),
      ended_at: localToRfc3339(end),
      break_minutes: Number(breakMin) || 0,
      notes: notes.trim() || null,
    };
    setBusy(true);
    try {
      if (entry) await me.editTime(entry.id, body);
      else await me.addTime(body);
      toast.success(entry ? "Time updated" : "Time added");
      onDone();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save that time"));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!entry || !confirm("Delete this time?")) return;
    setBusy(true);
    try {
      await me.deleteTime(entry.id);
      toast.success("Time deleted");
      onDone();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't delete that time"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>
          {entry ? "Change this time" : "Add time I forgot"}
        </DialogTitle>
        <DialogDescription>
          The office will see it and approve it with the rest of your week.
        </DialogDescription>
      </DialogHeader>
      <div className="space-y-4">
        <label className={label}>
          What were you working on?
          <WorkSelect work={work} value={target} onChange={setTarget} />
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
        {entry && (
          <Button
            variant="ghost"
            onClick={remove}
            disabled={busy}
            className="text-bad sm:mr-auto"
          >
            Delete
          </Button>
        )}
        <Button onClick={save} disabled={busy}>
          {busy ? "Saving…" : entry ? "Save" : "Add time"}
        </Button>
      </DialogFooter>
    </>
  );
}

// ---- weeks ------------------------------------------------------------------

function MyWeeks({ hours }: { hours: WeekHours[] }) {
  const rows = [...hours].sort((a, b) => b.week_of.localeCompare(a.week_of));
  return (
    <Card className="overflow-hidden">
      <div className="border-b border-line px-5 py-4">
        <h2 className="font-display text-lg font-bold">My weeks</h2>
        <p className="text-sm text-ink-3">
          The last eight weeks. Gross pay is before taxes.
        </p>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="text-left text-xs font-bold uppercase tracking-wide text-ink-3">
            <tr className="border-b border-line">
              <th className="px-5 py-3">Week of</th>
              <th className="px-3 py-3 text-right">Days</th>
              <th className="px-3 py-3 text-right">Hours</th>
              <th className="px-3 py-3 text-right">Regular</th>
              <th className="px-3 py-3 text-right">OT 1.5×</th>
              <th className="px-3 py-3 text-right">DT 2×</th>
              <th className="px-3 py-3 text-right">Waiting approval</th>
              <th className="px-5 py-3 text-right">Gross</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-line tabular-nums">
            {rows.map((w) => (
              <tr key={w.week_of}>
                <td className="px-5 py-3">{fmtDate(w.week_of)}</td>
                <td className="px-3 py-3 text-right">{w.days_worked}</td>
                <td className="px-3 py-3 text-right font-semibold">
                  {hm(w.minutes)}
                </td>
                <td className="px-3 py-3 text-right">
                  {hm(w.regular_minutes)}
                </td>
                <td className="px-3 py-3 text-right">
                  {w.overtime_minutes ? hm(w.overtime_minutes) : "—"}
                </td>
                <td className="px-3 py-3 text-right">
                  {w.double_minutes ? hm(w.double_minutes) : "—"}
                </td>
                <td className="px-3 py-3 text-right">
                  {w.unapproved_minutes ? (
                    <span className="text-warn">
                      {hm(w.unapproved_minutes)}
                    </span>
                  ) : (
                    "—"
                  )}
                </td>
                <td className="px-5 py-3 text-right font-semibold">
                  {money(w.gross_cents)}
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={8} className="px-5 py-10 text-center text-ink-3">
                  No hours yet.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </Card>
  );
}

// ---- time off ---------------------------------------------------------------

const TIME_OFF_KINDS: TimeOff["kind"][] = [
  "vacation",
  "sick",
  "personal",
  "unpaid",
];

const TIME_OFF_TONE: Record<
  TimeOff["status"],
  "warn" | "good" | "bad" | "neutral"
> = {
  pending: "warn",
  approved: "good",
  denied: "bad",
  cancelled: "neutral",
};

function TimeOffSection() {
  const [list, setList] = useState<TimeOff[]>([]);
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [kind, setKind] = useState<TimeOff["kind"]>("vacation");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const today = useMemo(() => isoDate(new Date()), []);

  const load = useCallback(() => {
    me.timeOff()
      .then(setList)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function submit() {
    if (!start || !end) {
      toast.error("Pick the first and last day");
      return;
    }
    setBusy(true);
    try {
      await me.requestTimeOff({
        starts_on: start,
        ends_on: end,
        kind,
        reason: reason.trim() || undefined,
      });
      toast.success("Request sent to the office");
      setStart("");
      setEnd("");
      setReason("");
      load();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't send the request"));
    } finally {
      setBusy(false);
    }
  }

  async function cancel(r: TimeOff) {
    if (!confirm("Cancel this time off request?")) return;
    try {
      await me.cancelTimeOff(r.id);
      toast.success("Request cancelled");
      load();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't cancel it"));
    }
  }

  return (
    <Card className="p-5">
      <h2 className="font-display text-lg font-bold">Time off</h2>
      <div className="mt-4 grid grid-cols-2 gap-3">
        <label className={label}>
          First day
          <input
            className={field}
            type="date"
            value={start}
            onChange={(e) => {
              setStart(e.target.value);
              if (!end || end < e.target.value) setEnd(e.target.value);
            }}
          />
        </label>
        <label className={label}>
          Last day
          <input
            className={field}
            type="date"
            value={end}
            min={start || undefined}
            onChange={(e) => setEnd(e.target.value)}
          />
        </label>
        <label className={label}>
          Kind
          <select
            className={field}
            value={kind}
            onChange={(e) => setKind(e.target.value as TimeOff["kind"])}
          >
            {TIME_OFF_KINDS.map((k) => (
              <option key={k} value={k}>
                {k[0].toUpperCase() + k.slice(1)}
              </option>
            ))}
          </select>
        </label>
        <label className={label}>
          Reason (optional)
          <input
            className={field}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
          />
        </label>
      </div>
      <div className="mt-3 flex justify-end">
        <Button onClick={submit} disabled={busy}>
          {busy ? "Sending…" : "Ask for time off"}
        </Button>
      </div>

      <div className="mt-5 divide-y divide-line border-t border-line">
        {list.map((r) => {
          const cancellable =
            r.status === "pending" ||
            (r.status === "approved" && r.starts_on > today);
          return (
            <div key={r.id} className="flex items-center gap-3 py-3">
              <div className="min-w-0 flex-1">
                <div className="font-semibold">
                  {fmtDate(r.starts_on)}
                  {r.ends_on !== r.starts_on && <> – {fmtDate(r.ends_on)}</>}
                </div>
                <div className="text-sm text-ink-3">
                  {r.kind} · {r.days} day{r.days === 1 ? "" : "s"}
                  {r.reason && <> · {r.reason}</>}
                  {r.review_note && <> · Office: {r.review_note}</>}
                </div>
              </div>
              <Badge tone={TIME_OFF_TONE[r.status]}>{r.status}</Badge>
              {cancellable && (
                <button
                  onClick={() => cancel(r)}
                  className="rounded-lg border border-line px-2 py-1 text-xs font-semibold text-ink-2 hover:border-bad"
                >
                  Cancel
                </button>
              )}
            </div>
          );
        })}
        {list.length === 0 && (
          <p className="py-6 text-center text-sm text-ink-3">
            No time off requests yet.
          </p>
        )}
      </div>
    </Card>
  );
}

const SHIFT_KIND_LABELS: Record<Shift["kind"], string> = {
  work: "Work",
  on_call: "On call",
  training: "Training",
};

function UpcomingShifts() {
  const [shifts, setShifts] = useState<Shift[]>([]);
  useEffect(() => {
    const today = new Date();
    me.shifts({ from: isoDate(today), to: isoDate(addDays(today, 13)) })
      .then(setShifts)
      .catch(() => undefined);
  }, []);
  return (
    <Card className="p-5">
      <h2 className="font-display text-lg font-bold">My upcoming shifts</h2>
      <p className="text-sm text-ink-3">The next two weeks.</p>
      <div className="mt-4 divide-y divide-line">
        {shifts.map((s) => (
          <div key={s.id} className="flex items-center gap-3 py-3">
            <div className="w-28 shrink-0 font-semibold">
              {fmtDay(s.starts_at)}
            </div>
            <div className="min-w-0 flex-1 text-sm">
              {fmtTime(s.starts_at)} – {fmtTime(s.ends_at)}
              {s.notes && <span className="text-ink-3"> · {s.notes}</span>}
            </div>
            <Badge
              tone={
                s.kind === "on_call"
                  ? "warn"
                  : s.kind === "training"
                    ? "info"
                    : "neutral"
              }
            >
              {SHIFT_KIND_LABELS[s.kind]}
            </Badge>
          </div>
        ))}
        {shifts.length === 0 && (
          <p className="py-6 text-center text-sm text-ink-3">
            Nothing on the schedule.
          </p>
        )}
      </div>
    </Card>
  );
}

// ---- mileage & receipts -----------------------------------------------------

const CATEGORY_LABELS: Partial<Record<ExpenseCategory, string>> = {
  materials: "Materials & parts",
  fuel: "Fuel",
  equipment: "Tools & equipment",
  repairs: "Repairs",
  vehicle: "Vehicle",
  other: "Other",
};

function catLabel(c: ExpenseCategory) {
  return CATEGORY_LABELS[c] ?? c[0].toUpperCase() + c.slice(1);
}

function ExpensesSection({ work }: { work: WorkOption[] }) {
  const [list, setList] = useState<Expense[]>([]);
  const [mode, setMode] = useState<"trip" | "receipt">("trip");
  const [date, setDate] = useState(() => isoDate(new Date()));
  const [wo, setWo] = useState("");
  const [busy, setBusy] = useState(false);
  // trip
  const [useOdo, setUseOdo] = useState(false);
  const [miles, setMiles] = useState("");
  const [odoStart, setOdoStart] = useState("");
  const [odoEnd, setOdoEnd] = useState("");
  const [roundTrip, setRoundTrip] = useState(false);
  const [vehicle, setVehicle] = useState<"personal" | "company">("personal");
  const [purpose, setPurpose] = useState("");
  // receipt
  const [category, setCategory] = useState<ExpenseCategory>("materials");
  const [vendor, setVendor] = useState("");
  const [amount, setAmount] = useState("");
  const [billable, setBillable] = useState(false);
  const [ownMoney, setOwnMoney] = useState(false);
  const [file, setFile] = useState<File | null>(null);
  const [fileKey, setFileKey] = useState(0);

  const load = useCallback(() => {
    const now = new Date();
    me.expenses({
      from: isoDate(new Date(now.getFullYear(), now.getMonth(), 1)),
      to: isoDate(new Date(now.getFullYear(), now.getMonth() + 1, 0)),
    })
      .then(setList)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function submit() {
    const woTarget = targetFrom(wo);
    let body: ExpenseInput;
    if (mode === "trip") {
      const details: Record<string, unknown> = { round_trip: roundTrip };
      if (purpose.trim()) details.purpose = purpose.trim();
      let m: number | null = null;
      if (useOdo) {
        const a = Number(odoStart);
        const b = Number(odoEnd);
        if (!odoStart || !odoEnd || !(b > a)) {
          toast.error("Enter both odometer readings (end higher than start)");
          return;
        }
        details.odometer_start = a;
        details.odometer_end = b;
      } else {
        const n = Number(miles);
        if (!miles || !(n > 0)) {
          toast.error("How many miles?");
          return;
        }
        m = roundTrip ? n * 2 : n;
      }
      body = {
        incurred_on: date,
        category: "mileage",
        description: purpose.trim() || "Trip",
        miles: m,
        vehicle,
        details,
        maintenance_ticket_id: woTarget?.maintenance_ticket_id ?? null,
      };
    } else {
      const cents = toCents(amount);
      if (!cents || cents <= 0) {
        toast.error("Enter the amount on the receipt");
        return;
      }
      body = {
        incurred_on: date,
        category,
        vendor: vendor.trim() || null,
        description: vendor.trim() || catLabel(category),
        amount_cents: cents,
        billable_to_owner: billable,
        reimbursable: ownMoney,
        maintenance_ticket_id: woTarget?.maintenance_ticket_id ?? null,
      };
    }
    setBusy(true);
    try {
      const created = await me.addExpense(body);
      if (mode === "receipt" && file) {
        try {
          await expenses.uploadReceipt(created.id, file);
        } catch (e) {
          toast.error(errMsg(e, "The receipt photo didn't upload"));
        }
      }
      toast.success(
        mode === "trip"
          ? `Trip logged — ${created.miles ?? ""} miles, ${money(created.amount_cents)}`
          : `Receipt logged — ${money(created.amount_cents)}`
      );
      setMiles("");
      setOdoStart("");
      setOdoEnd("");
      setPurpose("");
      setVendor("");
      setAmount("");
      setFile(null);
      setFileKey((k) => k + 1);
      load();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save that"));
    } finally {
      setBusy(false);
    }
  }

  const total = list.reduce((s, e) => s + e.amount_cents, 0);

  return (
    <Card className="p-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="font-display text-lg font-bold">
            Mileage &amp; receipts
          </h2>
          <p className="text-sm text-ink-3">
            Log a trip or snap a receipt so it gets paid back or billed.
          </p>
        </div>
        <div className="flex gap-1">
          {(["trip", "receipt"] as const).map((m) => (
            <button
              key={m}
              onClick={() => setMode(m)}
              className={`rounded-lg px-3 py-1.5 text-sm font-semibold ${
                mode === m
                  ? "bg-accent-soft text-accent-2"
                  : "text-ink-3 hover:bg-surface-2"
              }`}
            >
              {m === "trip" ? "A trip" : "A receipt"}
            </button>
          ))}
        </div>
      </div>

      <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <label className={label}>
          Date
          <input
            className={field}
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
        </label>
        <label className={`${label} lg:col-span-3`}>
          Work order (optional)
          <WorkSelect
            work={work}
            value={wo}
            onChange={setWo}
            includeOther={false}
            only="work_order"
            placeholder="Not for a work order"
          />
        </label>

        {mode === "trip" ? (
          <>
            {useOdo ? (
              <>
                <label className={label}>
                  Odometer start
                  <input
                    className={field}
                    type="number"
                    inputMode="decimal"
                    value={odoStart}
                    onChange={(e) => setOdoStart(e.target.value)}
                  />
                </label>
                <label className={label}>
                  Odometer end
                  <input
                    className={field}
                    type="number"
                    inputMode="decimal"
                    value={odoEnd}
                    onChange={(e) => setOdoEnd(e.target.value)}
                  />
                </label>
              </>
            ) : (
              <label className={label}>
                Miles
                <input
                  className={field}
                  type="number"
                  inputMode="decimal"
                  min={0}
                  step="0.1"
                  value={miles}
                  onChange={(e) => setMiles(e.target.value)}
                />
              </label>
            )}
            <label className={label}>
              Vehicle
              <select
                className={field}
                value={vehicle}
                onChange={(e) =>
                  setVehicle(e.target.value as "personal" | "company")
                }
              >
                <option value="personal">My own vehicle</option>
                <option value="company">Company vehicle</option>
              </select>
            </label>
            <label className={`${label} ${useOdo ? "" : "sm:col-span-2"}`}>
              Where to / why (optional)
              <input
                className={field}
                placeholder="e.g. Home Depot for parts"
                value={purpose}
                onChange={(e) => setPurpose(e.target.value)}
              />
            </label>
            <div className="flex flex-wrap items-center gap-4 text-sm sm:col-span-2 lg:col-span-4">
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={useOdo}
                  onChange={(e) => setUseOdo(e.target.checked)}
                />
                Use odometer readings
              </label>
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={roundTrip}
                  onChange={(e) => setRoundTrip(e.target.checked)}
                />
                Round trip (counts the miles twice)
              </label>
            </div>
          </>
        ) : (
          <>
            <label className={label}>
              What for
              <select
                className={field}
                value={category}
                onChange={(e) => setCategory(e.target.value as ExpenseCategory)}
              >
                {EXPENSE_CATEGORIES.filter(
                  (c) => c !== "mileage" && c !== "payroll"
                ).map((c) => (
                  <option key={c} value={c}>
                    {catLabel(c)}
                  </option>
                ))}
              </select>
            </label>
            <label className={label}>
              Store / vendor
              <input
                className={field}
                value={vendor}
                onChange={(e) => setVendor(e.target.value)}
              />
            </label>
            <label className={label}>
              Amount
              <input
                className={field}
                inputMode="decimal"
                placeholder="$0.00"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
              />
            </label>
            <label className={label}>
              Receipt photo (optional)
              <input
                key={fileKey}
                className="text-sm text-ink-2"
                type="file"
                accept="image/*,application/pdf"
                onChange={(e) => setFile(e.target.files?.[0] ?? null)}
              />
            </label>
            <div className="flex flex-wrap items-center gap-4 text-sm sm:col-span-2 lg:col-span-4">
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={billable}
                  onChange={(e) => setBillable(e.target.checked)}
                />
                Bill this to the owner
              </label>
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={ownMoney}
                  onChange={(e) => setOwnMoney(e.target.checked)}
                />
                I paid with my own money (pay me back)
              </label>
            </div>
          </>
        )}
      </div>
      <div className="mt-3 flex justify-end">
        <Button onClick={submit} disabled={busy}>
          {busy ? "Saving…" : mode === "trip" ? "Log trip" : "Log receipt"}
        </Button>
      </div>

      <div className="mt-5 border-t border-line pt-4">
        <div className="mb-2 flex items-center justify-between text-sm">
          <span className="font-semibold">This month</span>
          <span className="text-ink-3">{money(total)}</span>
        </div>
        <div className="divide-y divide-line">
          {list.map((e) => (
            <ExpenseRow key={e.id} expense={e} onChanged={load} />
          ))}
          {list.length === 0 && (
            <p className="py-6 text-center text-sm text-ink-3">
              Nothing logged this month.
            </p>
          )}
        </div>
      </div>
    </Card>
  );
}

function ExpenseRow({
  expense: e,
  onChanged,
}: {
  expense: Expense;
  onChanged: () => void;
}) {
  const [busy, setBusy] = useState(false);

  async function upload(file: File | undefined) {
    if (!file) return;
    setBusy(true);
    try {
      await expenses.uploadReceipt(e.id, file);
      toast.success("Receipt added");
      onChanged();
    } catch (err) {
      toast.error(errMsg(err, "The receipt didn't upload"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-3 py-3 text-sm">
      <div className="w-20 shrink-0 text-ink-3">{fmtDate(e.incurred_on)}</div>
      <div className="min-w-0 flex-1">
        <div className="font-semibold">
          {e.category === "mileage"
            ? `${e.miles ?? 0} miles${e.vehicle === "personal" ? " · own vehicle" : ""}`
            : e.vendor || catLabel(e.category)}
        </div>
        <div className="text-xs text-ink-3">
          {e.description}
          {e.work_order_title && <> · {e.work_order_title}</>}
        </div>
      </div>
      <div className="flex flex-wrap gap-1">
        {e.reimbursed ? (
          <Badge tone="good">paid back</Badge>
        ) : (
          e.reimbursable && <Badge tone="warn">to pay back</Badge>
        )}
        {e.billable_to_owner && (
          <Badge tone="info">{e.billed ? "billed" : "bill owner"}</Badge>
        )}
        {e.receipts > 0 && (
          <Badge>
            {e.receipts} receipt{e.receipts === 1 ? "" : "s"}
          </Badge>
        )}
      </div>
      <div className="w-24 text-right font-semibold tabular-nums">
        {money(e.amount_cents)}
      </div>
      <label
        className={`cursor-pointer rounded-lg border border-line px-2 py-1 text-xs font-semibold text-ink-2 hover:border-accent ${
          busy ? "opacity-50" : ""
        }`}
      >
        {busy ? "Uploading…" : "+ receipt"}
        <input
          type="file"
          className="hidden"
          accept="image/*,application/pdf"
          disabled={busy}
          onChange={(ev) => upload(ev.target.files?.[0])}
        />
      </label>
    </div>
  );
}
