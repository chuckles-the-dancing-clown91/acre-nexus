"use client";

// My time: self-service for anyone with an employee profile. The clock,
// missed punches, this week's time, weekly totals, time off, upcoming shifts,
// and mileage and receipts. Built for a phone first: a tech in the field
// clocks in and logs a trip from here.

import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Car,
  ChevronLeft,
  ChevronRight,
  Clock,
  LogIn,
  LogOut,
  MapPin,
  Pencil,
  Plus,
  Receipt as ReceiptIcon,
  Send,
  Trash2,
  Upload,
  UserX,
} from "lucide-react";
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
  type Expense,
  type ExpenseCategory,
  type ExpenseInput,
  type Shift,
  type TimeEntry,
  type TimeOff,
  type WeekHours,
  type WorkOption,
} from "@/lib/backoffice";
import {
  OTHER_KINDS,
  addDays,
  errMsg,
  fmtDate,
  fmtDay,
  fmtTime,
  plural,
  targetFrom,
  targetValue,
  useReady,
  workLabel,
} from "@/lib/money-extra";
import { Badge, type Tone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs } from "@/components/ui/data-table";
import { fieldClass } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const check = "size-5 accent-[var(--accent)]";
// Tall fields with 16px text so phones don't zoom in on focus.
const full = cn(fieldClass, "h-12 w-full text-base sm:h-11 sm:text-[13px]");

function notOnTeam(e: unknown) {
  return e instanceof ApiError && e.status === 403;
}

// ---- work picker ------------------------------------------------------------

function optionText(o: WorkOption) {
  return o.property_name && o.kind !== "property"
    ? `${o.label} · ${o.property_name}`
    : o.label;
}

function WorkSelect({
  work,
  value,
  onChange,
  includeOther = true,
  only,
  placeholder = "Pick what you're working on",
  label,
}: {
  work: WorkOption[];
  value: string;
  onChange: (v: string) => void;
  includeOther?: boolean;
  only?: WorkOption["kind"];
  placeholder?: string;
  label: string;
}) {
  const list = only ? work.filter((w) => w.kind === only) : work;
  const groups: [string, WorkOption[]][] = [
    ["Assigned to you", list.filter((w) => w.mine)],
    ["Work orders", list.filter((w) => !w.mine && w.kind === "work_order")],
    ["Rehab projects", list.filter((w) => !w.mine && w.kind === "project")],
    ["Properties", list.filter((w) => !w.mine && w.kind === "property")],
  ];
  return (
    <select
      aria-label={label}
      className={full}
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

// ---- page -------------------------------------------------------------------

export default function MyTimePage() {
  const ready = useReady();
  const qc = useQueryClient();
  const [week, setWeek] = useState(() => weekStart(new Date()));
  const [today] = useState(() => new Date());

  const clock = useQuery({
    queryKey: ["me", "clock"],
    queryFn: me.clock,
    enabled: ready,
    retry: (n, e) => !notOnTeam(e) && n < 2,
  });
  const onTeam = !notOnTeam(clock.error);
  const work = useQuery({
    queryKey: ["me", "work"],
    queryFn: me.work,
    enabled: ready && onTeam,
  });
  const weekFrom = isoDate(week);
  const weekTo = isoDate(addDays(week, 6));
  const entries = useQuery({
    queryKey: ["me", "time", weekFrom, weekTo],
    queryFn: () => me.time({ from: weekFrom, to: weekTo }),
    enabled: ready && onTeam,
  });
  const recent = useQuery({
    queryKey: ["me", "time", "recent"],
    queryFn: () =>
      me.time({ from: isoDate(addDays(today, -90)), to: isoDate(today) }),
    enabled: ready && onTeam,
  });
  const hours = useQuery({
    queryKey: ["me", "hours"],
    queryFn: () => {
      const thisWeek = weekStart(today);
      return me.hours({
        from: isoDate(addDays(thisWeek, -7 * 7)),
        to: isoDate(addDays(thisWeek, 6)),
      });
    },
    enabled: ready && onTeam,
  });

  const refreshAll = () => void qc.invalidateQueries({ queryKey: ["me"] });
  const workList = work.data ?? [];

  if (!onTeam)
    return (
      <div className="space-y-6">
        <Header />
        <Panel>
          <EmptyState
            icon={<UserX />}
            title="You're not on the team yet"
            description="Ask the office to add your employee profile, then come back here to clock in."
          />
        </Panel>
      </div>
    );

  const missed = (recent.data ?? []).filter(
    (e) => e.needs_review && !e.claimed_end
  );

  return (
    <div className="space-y-6">
      <Header />
      {clock.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          <p role="alert">{clock.error.message}</p>
        </Panel>
      )}

      {clock.data ? (
        <ClockCard clock={clock.data} work={workList} onChanged={refreshAll} />
      ) : (
        !clock.error && <Skeleton className="h-64 rounded-2xl" />
      )}

      {clock.data && clock.data.missed_punches > 0 && missed.length > 0 && (
        <MissedPunches entries={missed} onChanged={refreshAll} />
      )}

      <WeekEntries
        week={week}
        setWeek={setWeek}
        entries={entries.data}
        work={workList}
        onChanged={refreshAll}
      />

      <MyWeeks hours={hours.data} />

      <div className="grid gap-6 lg:grid-cols-2">
        <TimeOffSection enabled={ready} />
        <UpcomingShifts enabled={ready} />
      </div>

      <ExpensesSection work={workList} enabled={ready} />
    </div>
  );
}

function Header() {
  return (
    <PageHeader
      eyebrow="Team"
      title="My time"
      description="Clock in and out, fix a missed punch, ask for time off, and log miles and receipts."
    />
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
    <span
      className="figure text-[44px] leading-none font-semibold text-fg sm:text-[52px]"
      aria-label={`${h} hours ${m} minutes on the clock`}
    >
      {h}:{String(m).padStart(2, "0")}:{String(s).padStart(2, "0")}
    </span>
  );
}

const QUICK = [
  ["travel", "Travel"],
  ["shop", "Shop time"],
  ["admin", "Office time"],
] as const;

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
    <div className="flex gap-6">
      <div>
        <div className="eyebrow">Today</div>
        <div className="figure mt-1 text-[20px] font-semibold text-fg">
          {hm(clock.today_minutes)}
        </div>
      </div>
      <div>
        <div className="eyebrow">This week</div>
        <div className="figure mt-1 text-[20px] font-semibold text-fg">
          {hm(clock.week_minutes)}
        </div>
      </div>
    </div>
  );

  return (
    <Panel className={cn("p-5 sm:p-6", open && "border-good/50")}>
      {open ? (
        <div className="space-y-5">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
            <div className="min-w-0 space-y-2">
              <Badge tone="good" dot>
                On the clock
              </Badge>
              <div className="text-[20px] leading-snug font-semibold text-fg">
                {workLabel(open)}
              </div>
              <div className="text-[13px] text-fg-3">
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
          <div className="grid gap-3 border-t border-line pt-5 sm:grid-cols-[9rem_1fr_auto] sm:items-end">
            <Labeled label="Break (minutes)">
              <input
                className={full}
                type="number"
                min={0}
                inputMode="numeric"
                placeholder="0"
                value={breakMin}
                onChange={(e) => setBreakMin(e.target.value)}
              />
            </Labeled>
            <Labeled label="Note (optional)">
              <input
                className={full}
                placeholder="What got done"
                value={note}
                onChange={(e) => setNote(e.target.value)}
              />
            </Labeled>
            <Button
              size="lg"
              variant="danger"
              loading={busy}
              onClick={clockOut}
              className="w-full sm:w-auto"
            >
              <LogOut />
              Clock out
            </Button>
          </div>
        </div>
      ) : (
        <div className="space-y-5">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
            <div>
              <Badge>Not clocked in</Badge>
              <div className="mt-2 text-[20px] font-semibold text-fg">
                What are you working on?
              </div>
            </div>
            {totals}
          </div>
          <div className="space-y-3">
            <WorkSelect
              label="What you're working on"
              work={work}
              value={target}
              onChange={setTarget}
              includeOther={false}
            />
            <div className="flex flex-wrap gap-2">
              {QUICK.map(([k, name]) => (
                <button
                  key={k}
                  type="button"
                  aria-pressed={target === k}
                  onClick={() => setTarget(k)}
                  className={cn(
                    "h-10 rounded-full border px-4 text-[13px] font-medium transition",
                    target === k
                      ? "border-accent bg-accent/10 text-accent"
                      : "border-line text-fg-2 hover:bg-fill-2"
                  )}
                >
                  {name}
                </button>
              ))}
            </div>
            <div className="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
              <Labeled label="Note (optional)">
                <input
                  className={full}
                  placeholder="Anything the office should know"
                  value={note}
                  onChange={(e) => setNote(e.target.value)}
                />
              </Labeled>
              <Button
                size="lg"
                loading={busy}
                disabled={!target}
                onClick={clockIn}
                className="w-full sm:w-auto"
              >
                <LogIn />
                Clock in
              </Button>
            </div>
          </div>
        </div>
      )}
      {(clock.records_location || clock.overtime_rule) && (
        <div className="mt-5 space-y-1 text-xs text-fg-3">
          {clock.records_location && (
            <p className="flex items-center gap-1.5">
              <MapPin className="size-3.5" />
              We note where you are only when you clock in or out.
            </p>
          )}
          {clock.overtime_rule && <p>{clock.overtime_rule}</p>}
        </div>
      )}
    </Panel>
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
    <Panel className="border-warn/40 bg-warn/5 p-5">
      <h2 className="text-[15px] font-semibold text-warn">
        You forgot to clock out
      </h2>
      <p className="mt-0.5 mb-4 text-[13px] text-fg-2">
        Tell us when you finished and the office will settle it.
      </p>
      <div className="space-y-3">
        {entries.map((e) => (
          <MissedRow key={e.id} entry={e} onChanged={onChanged} />
        ))}
      </div>
    </Panel>
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
      toast.success("Thanks. The office will take a look.");
      onChanged();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save that"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="grid gap-3 rounded-xl border border-line bg-surface p-3 sm:grid-cols-[1fr_auto_auto_auto] sm:items-end">
      <div className="min-w-0">
        <div className="text-[14px] font-medium text-fg">
          {workLabel(entry)}
        </div>
        <div className="text-[13px] text-fg-3">
          Started {fmtDay(entry.started_at)} at {fmtTime(entry.started_at)}
          {entry.missed_punch_reason && <> · {entry.missed_punch_reason}</>}
        </div>
      </div>
      <Labeled label="When did you finish?">
        <input
          className={full}
          type="datetime-local"
          min={rfc3339ToLocal(entry.started_at)}
          value={end}
          onChange={(e) => setEnd(e.target.value)}
        />
      </Labeled>
      <Labeled label="Note">
        <input
          className={full}
          placeholder="Optional"
          value={note}
          onChange={(e) => setNote(e.target.value)}
        />
      </Labeled>
      <Button onClick={save} loading={busy} className="h-12 sm:h-11">
        <Send />
        Send
      </Button>
    </div>
  );
}

// ---- this week --------------------------------------------------------------

function entryStatus(e: TimeEntry) {
  if (!e.ended_at && !e.needs_review)
    return (
      <Badge tone="accent" dot>
        on the clock
      </Badge>
    );
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
  entries: TimeEntry[] | undefined;
  work: WorkOption[];
  onChanged: () => void;
}) {
  const [editing, setEditing] = useState<TimeEntry | null>(null);
  const [adding, setAdding] = useState(false);
  const list = entries ?? [];
  const total = list.reduce((s, e) => s + e.minutes, 0);
  const end = addDays(week, 6);
  const short = (d: Date) =>
    d.toLocaleDateString([], { month: "short", day: "numeric" });

  return (
    <Panel className="overflow-hidden">
      <div className="flex flex-col gap-3 border-b border-line px-5 py-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h2 className="text-[15px] font-semibold text-fg">This week</h2>
          <p className="text-[13px] text-fg-3">
            {short(week)} to {short(end)} ·{" "}
            <span className="figure text-fg-2">{hm(total)}</span> hours
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            size="icon"
            variant="secondary"
            aria-label="Previous week"
            onClick={() => setWeek(addDays(week, -7))}
          >
            <ChevronLeft />
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setWeek(weekStart(new Date()))}
          >
            This week
          </Button>
          <Button
            size="icon"
            variant="secondary"
            aria-label="Next week"
            onClick={() => setWeek(addDays(week, 7))}
          >
            <ChevronRight />
          </Button>
          <Button
            size="sm"
            className="ml-auto sm:ml-2"
            onClick={() => setAdding(true)}
          >
            <Plus />
            Add time I forgot
          </Button>
        </div>
      </div>
      {!entries ? (
        <div className="space-y-2 p-4">
          {Array.from({ length: 3 }, (_, i) => (
            <Skeleton key={i} className="h-14" />
          ))}
        </div>
      ) : list.length === 0 ? (
        <EmptyState
          icon={<Clock />}
          title="No time this week"
          className="py-10"
        />
      ) : (
        <ul className="divide-y divide-line">
          {list.map((e) => (
            <li key={e.id} className="flex items-center gap-3 px-5 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                  <span className="truncate text-[14px] font-medium text-fg">
                    {workLabel(e)}
                  </span>
                  {entryStatus(e)}
                </div>
                <div className="mt-0.5 text-xs text-fg-3">
                  {fmtDay(e.started_at)} · {fmtTime(e.started_at)} to{" "}
                  {fmtTime(e.ended_at)}
                  {e.break_minutes ? ` · ${e.break_minutes}m break` : ""}
                </div>
                {e.notes && (
                  <div className="mt-0.5 truncate text-xs text-fg-3">
                    {e.notes}
                  </div>
                )}
              </div>
              <span className="figure shrink-0 text-[15px] font-semibold text-fg">
                {hm(e.minutes)}
              </span>
              {canChange(e) ? (
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label="Change this time"
                  onClick={() => setEditing(e)}
                >
                  <Pencil />
                </Button>
              ) : (
                <span className="size-9 shrink-0" aria-hidden />
              )}
            </li>
          ))}
        </ul>
      )}

      {(adding || editing) && (
        <EntryDialog
          key={editing?.id ?? "new"}
          entry={editing}
          work={work}
          onClose={() => {
            setAdding(false);
            setEditing(null);
          }}
          onDone={() => {
            setAdding(false);
            setEditing(null);
            onChanged();
          }}
        />
      )}
    </Panel>
  );
}

function EntryDialog({
  entry,
  work,
  onClose,
  onDone,
}: {
  entry: TimeEntry | null;
  work: WorkOption[];
  onClose: () => void;
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
    if (!entry || !window.confirm("Delete this time?")) return;
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
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto p-5 sm:p-6">
        <DialogTitle className="pr-8 text-[17px] font-semibold">
          {entry ? "Change this time" : "Add time I forgot"}
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          The office sees it and approves it with the rest of your week.
        </DialogDescription>
        <div className="mt-5 space-y-4">
          <Labeled label="What were you working on?">
            <WorkSelect
              label="What you worked on"
              work={work}
              value={target}
              onChange={setTarget}
            />
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
              className={cn(
                fieldClass,
                "min-h-[72px] w-full text-base sm:text-[13px]"
              )}
              rows={2}
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
            />
          </Labeled>
        </div>
        <div className="mt-6 flex flex-wrap items-center gap-2">
          {entry && (
            <Button
              variant="ghost"
              onClick={remove}
              disabled={busy}
              className="text-bad hover:text-bad"
            >
              <Trash2 />
              Delete
            </Button>
          )}
          <div className="ml-auto flex gap-2">
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button onClick={save} loading={busy}>
              {entry ? "Save" : "Add time"}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// ---- weeks ------------------------------------------------------------------

function MyWeeks({ hours }: { hours: WeekHours[] | undefined }) {
  const rows = useMemo(
    () => [...(hours ?? [])].sort((a, b) => b.week_of.localeCompare(a.week_of)),
    [hours]
  );
  return (
    <Panel className="overflow-hidden">
      <PanelHeader
        title="My weeks"
        description="The last eight weeks. Gross pay is before taxes."
        className="pb-4"
      />
      {!hours ? (
        <div className="space-y-2 p-4 pt-0">
          <Skeleton className="h-14" />
          <Skeleton className="h-14" />
        </div>
      ) : rows.length === 0 ? (
        <p className="border-t border-line px-5 py-8 text-center text-[13px] text-fg-3">
          No hours yet.
        </p>
      ) : (
        <ul className="divide-y divide-line border-t border-line">
          {rows.map((w) => (
            <li key={w.week_of} className="px-5 py-3">
              <div className="flex items-baseline justify-between gap-3">
                <span className="text-[14px] font-medium text-fg">
                  Week of {fmtDate(w.week_of)}
                </span>
                <span className="figure text-[14px] font-semibold text-fg">
                  {money(w.gross_cents)}
                </span>
              </div>
              <div className="mt-0.5 flex flex-wrap gap-x-3 gap-y-0.5 text-xs text-fg-3">
                <span>{plural(w.days_worked, "day")}</span>
                <span>
                  <span className="figure text-fg-2">{hm(w.minutes)}</span>{" "}
                  hours
                </span>
                <span>regular {hm(w.regular_minutes)}</span>
                {w.overtime_minutes > 0 && (
                  <span>OT 1.5x {hm(w.overtime_minutes)}</span>
                )}
                {w.double_minutes > 0 && (
                  <span>DT 2x {hm(w.double_minutes)}</span>
                )}
                {w.unapproved_minutes > 0 && (
                  <span className="text-warn">
                    {hm(w.unapproved_minutes)} waiting approval
                  </span>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </Panel>
  );
}

// ---- time off ---------------------------------------------------------------

const TIME_OFF_KINDS: TimeOff["kind"][] = [
  "vacation",
  "sick",
  "personal",
  "unpaid",
];

const TIME_OFF_TONE: Record<TimeOff["status"], Tone> = {
  pending: "warn",
  approved: "good",
  denied: "bad",
  cancelled: "neutral",
};

function TimeOffSection({ enabled }: { enabled: boolean }) {
  const qc = useQueryClient();
  const list = useQuery({
    queryKey: ["me", "time-off"],
    queryFn: me.timeOff,
    enabled,
  });
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [kind, setKind] = useState<TimeOff["kind"]>("vacation");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [today] = useState(() => isoDate(new Date()));
  const reload = () =>
    void qc.invalidateQueries({ queryKey: ["me", "time-off"] });

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
      reload();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't send the request"));
    } finally {
      setBusy(false);
    }
  }

  async function cancel(r: TimeOff) {
    if (!window.confirm("Cancel this time off request?")) return;
    try {
      await me.cancelTimeOff(r.id);
      toast.success("Request cancelled");
      reload();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't cancel it"));
    }
  }

  return (
    <Panel className="p-5">
      <h2 className="text-[15px] font-semibold text-fg">Time off</h2>
      <div className="mt-4 grid grid-cols-2 gap-3">
        <Labeled label="First day">
          <input
            className={full}
            type="date"
            value={start}
            onChange={(e) => {
              setStart(e.target.value);
              if (!end || end < e.target.value) setEnd(e.target.value);
            }}
          />
        </Labeled>
        <Labeled label="Last day">
          <input
            className={full}
            type="date"
            value={end}
            min={start || undefined}
            onChange={(e) => setEnd(e.target.value)}
          />
        </Labeled>
        <Labeled label="Kind">
          <select
            className={full}
            value={kind}
            onChange={(e) => setKind(e.target.value as TimeOff["kind"])}
          >
            {TIME_OFF_KINDS.map((k) => (
              <option key={k} value={k}>
                {k[0].toUpperCase() + k.slice(1)}
              </option>
            ))}
          </select>
        </Labeled>
        <Labeled label="Reason (optional)">
          <input
            className={full}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
          />
        </Labeled>
      </div>
      <div className="mt-3 flex justify-end">
        <Button onClick={submit} loading={busy} className="w-full sm:w-auto">
          <Send />
          Ask for time off
        </Button>
      </div>

      <div className="mt-5 divide-y divide-line border-t border-line">
        {list.isLoading && <Skeleton className="mt-3 h-12" />}
        {list.data?.map((r) => {
          const cancellable =
            r.status === "pending" ||
            (r.status === "approved" && r.starts_on > today);
          return (
            <div key={r.id} className="flex items-center gap-3 py-3">
              <div className="min-w-0 flex-1">
                <div className="text-[14px] font-medium text-fg">
                  {fmtDate(r.starts_on)}
                  {r.ends_on !== r.starts_on && <> to {fmtDate(r.ends_on)}</>}
                </div>
                <div className="text-[13px] text-fg-3">
                  {r.kind} · {plural(r.days, "day")}
                  {r.reason && <> · {r.reason}</>}
                  {r.review_note && <> · Office: {r.review_note}</>}
                </div>
              </div>
              <Badge tone={TIME_OFF_TONE[r.status]}>{r.status}</Badge>
              {cancellable && (
                <Button size="sm" variant="ghost" onClick={() => cancel(r)}>
                  Cancel
                </Button>
              )}
            </div>
          );
        })}
        {list.data?.length === 0 && (
          <p className="py-6 text-center text-[13px] text-fg-3">
            No time off requests yet.
          </p>
        )}
      </div>
    </Panel>
  );
}

const SHIFT_KIND_LABELS: Record<Shift["kind"], string> = {
  work: "Work",
  on_call: "On call",
  training: "Training",
};

function UpcomingShifts({ enabled }: { enabled: boolean }) {
  const [today] = useState(() => new Date());
  const shifts = useQuery({
    queryKey: ["me", "shifts"],
    queryFn: () =>
      me.shifts({ from: isoDate(today), to: isoDate(addDays(today, 13)) }),
    enabled,
  });
  return (
    <Panel className="p-5">
      <h2 className="text-[15px] font-semibold text-fg">My upcoming shifts</h2>
      <p className="mt-0.5 text-[13px] text-fg-3">The next two weeks.</p>
      <div className="mt-4 divide-y divide-line">
        {shifts.isLoading && <Skeleton className="h-12" />}
        {shifts.data?.map((s) => (
          <div key={s.id} className="flex items-center gap-3 py-3">
            <div className="w-28 shrink-0 text-[14px] font-medium text-fg">
              {fmtDay(s.starts_at)}
            </div>
            <div className="min-w-0 flex-1 text-[13px] text-fg-2">
              <span className="figure">
                {fmtTime(s.starts_at)} to {fmtTime(s.ends_at)}
              </span>
              {s.notes && <span className="text-fg-3"> · {s.notes}</span>}
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
        {shifts.data?.length === 0 && (
          <p className="py-6 text-center text-[13px] text-fg-3">
            Nothing on the schedule.
          </p>
        )}
      </div>
    </Panel>
  );
}

// ---- mileage and receipts ---------------------------------------------------

const CATEGORY_LABELS: Partial<Record<ExpenseCategory, string>> = {
  materials: "Materials and parts",
  fuel: "Fuel",
  equipment: "Tools and equipment",
  repairs: "Repairs",
  vehicle: "Vehicle",
  other: "Other",
};

function catLabel(c: ExpenseCategory) {
  return CATEGORY_LABELS[c] ?? c[0].toUpperCase() + c.slice(1);
}

function ExpensesSection({
  work,
  enabled,
}: {
  work: WorkOption[];
  enabled: boolean;
}) {
  const qc = useQueryClient();
  const [range] = useState(() => {
    const now = new Date();
    return {
      from: isoDate(new Date(now.getFullYear(), now.getMonth(), 1)),
      to: isoDate(new Date(now.getFullYear(), now.getMonth() + 1, 0)),
    };
  });
  const list = useQuery({
    queryKey: ["me", "expenses", range.from],
    queryFn: () => me.expenses(range),
    enabled,
  });
  const reload = () =>
    void qc.invalidateQueries({ queryKey: ["me", "expenses"] });

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
          toast.error(
            "Enter both odometer readings, the end higher than the start"
          );
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
          ? `Trip logged · ${created.miles ?? ""} miles, ${money(created.amount_cents)}`
          : `Receipt logged · ${money(created.amount_cents)}`
      );
      setMiles("");
      setOdoStart("");
      setOdoEnd("");
      setPurpose("");
      setVendor("");
      setAmount("");
      setFile(null);
      setFileKey((k) => k + 1);
      reload();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save that"));
    } finally {
      setBusy(false);
    }
  }

  const rows = list.data ?? [];
  const total = rows.reduce((s, e) => s + e.amount_cents, 0);

  return (
    <Panel className="p-5">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h2 className="text-[15px] font-semibold text-fg">
            Mileage and receipts
          </h2>
          <p className="mt-0.5 text-[13px] text-fg-3">
            Log a trip or snap a receipt so it gets paid back or billed.
          </p>
        </div>
        <Tabs
          tabs={[
            ["trip", "A trip"],
            ["receipt", "A receipt"],
          ]}
          value={mode}
          onChange={setMode}
          className="w-fit"
        />
      </div>

      <div className="mt-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <Labeled label="Date">
          <input
            className={full}
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
        </Labeled>
        <Labeled
          label="Work order (optional)"
          className="sm:col-span-1 lg:col-span-3"
        >
          <WorkSelect
            label="Work order"
            work={work}
            value={wo}
            onChange={setWo}
            includeOther={false}
            only="work_order"
            placeholder="Not for a work order"
          />
        </Labeled>

        {mode === "trip" ? (
          <>
            {useOdo ? (
              <>
                <Labeled label="Odometer start">
                  <input
                    className={full}
                    type="number"
                    inputMode="decimal"
                    value={odoStart}
                    onChange={(e) => setOdoStart(e.target.value)}
                  />
                </Labeled>
                <Labeled label="Odometer end">
                  <input
                    className={full}
                    type="number"
                    inputMode="decimal"
                    value={odoEnd}
                    onChange={(e) => setOdoEnd(e.target.value)}
                  />
                </Labeled>
              </>
            ) : (
              <Labeled label="Miles">
                <input
                  className={full}
                  type="number"
                  inputMode="decimal"
                  min={0}
                  step="0.1"
                  value={miles}
                  onChange={(e) => setMiles(e.target.value)}
                />
              </Labeled>
            )}
            <Labeled label="Vehicle">
              <select
                className={full}
                value={vehicle}
                onChange={(e) =>
                  setVehicle(e.target.value as "personal" | "company")
                }
              >
                <option value="personal">My own vehicle</option>
                <option value="company">Company vehicle</option>
              </select>
            </Labeled>
            <Labeled
              label="Where to and why (optional)"
              className={useOdo ? undefined : "sm:col-span-2"}
            >
              <input
                className={full}
                placeholder="Home Depot for parts"
                value={purpose}
                onChange={(e) => setPurpose(e.target.value)}
              />
            </Labeled>
            <div className="flex flex-col gap-3 text-[14px] text-fg-2 sm:col-span-2 sm:flex-row sm:flex-wrap sm:gap-5 lg:col-span-4">
              <label className="flex items-center gap-2.5">
                <input
                  type="checkbox"
                  className={check}
                  checked={useOdo}
                  onChange={(e) => setUseOdo(e.target.checked)}
                />
                Use odometer readings
              </label>
              <label className="flex items-center gap-2.5">
                <input
                  type="checkbox"
                  className={check}
                  checked={roundTrip}
                  onChange={(e) => setRoundTrip(e.target.checked)}
                />
                Round trip (counts the miles twice)
              </label>
            </div>
          </>
        ) : (
          <>
            <Labeled label="What for">
              <select
                className={full}
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
            </Labeled>
            <Labeled label="Store or vendor">
              <input
                className={full}
                value={vendor}
                onChange={(e) => setVendor(e.target.value)}
              />
            </Labeled>
            <Labeled label="Amount">
              <input
                className={full}
                inputMode="decimal"
                placeholder="$0.00"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
              />
            </Labeled>
            <Labeled label="Receipt photo (optional)">
              <span
                className={cn(
                  full,
                  "flex cursor-pointer items-center gap-2 text-fg-2"
                )}
              >
                <Upload className="size-4 shrink-0 text-fg-3" />
                <span className="truncate">
                  {file ? file.name : "Take or pick a photo"}
                </span>
                <input
                  key={fileKey}
                  className="sr-only"
                  type="file"
                  accept="image/*,application/pdf"
                  onChange={(e) => setFile(e.target.files?.[0] ?? null)}
                />
              </span>
            </Labeled>
            <div className="flex flex-col gap-3 text-[14px] text-fg-2 sm:col-span-2 sm:flex-row sm:flex-wrap sm:gap-5 lg:col-span-4">
              <label className="flex items-center gap-2.5">
                <input
                  type="checkbox"
                  className={check}
                  checked={billable}
                  onChange={(e) => setBillable(e.target.checked)}
                />
                Bill this to the owner
              </label>
              <label className="flex items-center gap-2.5">
                <input
                  type="checkbox"
                  className={check}
                  checked={ownMoney}
                  onChange={(e) => setOwnMoney(e.target.checked)}
                />
                I paid with my own money (pay me back)
              </label>
            </div>
          </>
        )}
      </div>
      <div className="mt-4 flex justify-end">
        <Button
          size="lg"
          onClick={submit}
          loading={busy}
          className="w-full sm:w-auto"
        >
          {mode === "trip" ? <Car /> : <ReceiptIcon />}
          {mode === "trip" ? "Log trip" : "Log receipt"}
        </Button>
      </div>

      <div className="mt-5 border-t border-line pt-4">
        <div className="mb-1 flex items-center justify-between text-[13px]">
          <span className="font-medium text-fg">This month</span>
          <span className="figure text-fg-2">{money(total)}</span>
        </div>
        {list.isLoading && <Skeleton className="mt-2 h-12" />}
        <div className="divide-y divide-line">
          {rows.map((e) => (
            <ExpenseRow key={e.id} expense={e} onChanged={reload} />
          ))}
        </div>
        {list.data?.length === 0 && (
          <p className="py-6 text-center text-[13px] text-fg-3">
            Nothing logged this month.
          </p>
        )}
      </div>
    </Panel>
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
    <div className="flex items-center gap-3 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
          <span className="text-[14px] font-medium text-fg">
            {e.category === "mileage"
              ? `${e.miles ?? 0} miles${e.vehicle === "personal" ? " · own vehicle" : ""}`
              : e.vendor || catLabel(e.category)}
          </span>
          {e.reimbursed ? (
            <Badge tone="good">paid back</Badge>
          ) : (
            e.reimbursable && <Badge tone="warn">to pay back</Badge>
          )}
          {e.billable_to_owner && (
            <Badge tone="info">{e.billed ? "billed" : "bill owner"}</Badge>
          )}
          {e.receipts > 0 && <Badge>{plural(e.receipts, "receipt")}</Badge>}
        </div>
        <div className="mt-0.5 truncate text-xs text-fg-3">
          {fmtDate(e.incurred_on)} · {e.description}
          {e.work_order_title && <> · {e.work_order_title}</>}
        </div>
      </div>
      <span className="figure shrink-0 text-[14px] font-semibold text-fg">
        {money(e.amount_cents)}
      </span>
      <label
        className={cn(
          "flex size-10 shrink-0 cursor-pointer items-center justify-center rounded-xl text-fg-2 transition hover:bg-fill-2 hover:text-fg",
          busy && "pointer-events-none opacity-45"
        )}
        title="Add a receipt"
      >
        <Upload className="size-4" />
        <span className="sr-only">{busy ? "Uploading" : "Add a receipt"}</span>
        <input
          type="file"
          className="sr-only"
          accept="image/*,application/pdf"
          disabled={busy}
          onChange={(ev) => {
            const f = ev.target.files?.[0];
            ev.target.value = "";
            void upload(f);
          }}
        />
      </label>
    </div>
  );
}
