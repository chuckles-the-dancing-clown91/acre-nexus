"use client";

// Team: the office view of staff — profiles and pay, the weekly schedule, and
// time off requests. Gated by `team:read`; changes need `team:manage`. Pay and
// bill rates only come back with `payroll:read`.

import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { useAuth } from "@/lib/auth";
import {
  EMPLOYMENT_LABELS,
  hm,
  isoDate,
  localToRfc3339,
  money,
  team,
  toCents,
  weekStart,
  type Employee,
  type EmploymentType,
  type ProfileInput,
  type Shift,
  type ShiftInput,
  type TimeOff,
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

function fmtDate(ymd: string) {
  const [y, m, d] = ymd.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString([], {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function fmtTime(iso: string) {
  return new Date(iso).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

function hhmm(iso: string) {
  const d = new Date(iso);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

function dollars(cents: number | null | undefined) {
  return cents == null ? "" : (cents / 100).toFixed(2);
}

type Tab = "people" | "schedule" | "timeoff";

export default function TeamPage() {
  const { can } = useAuth();
  const read = can("team:read");
  const manage = can("team:manage");
  const [tab, setTab] = useState<Tab>("people");
  const [roster, setRoster] = useState<Employee[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  const loadRoster = useCallback(() => {
    team
      .roster()
      .then((r) => {
        setRoster(r);
        setLoaded(true);
      })
      .catch((e) => setError(errMsg(e)));
  }, []);

  useEffect(() => {
    if (!read) return;
    loadRoster();
  }, [read, loadRoster]);

  if (!read) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to the team. Ask an admin for the{" "}
          <span className="font-mono">team:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Team
          </h1>
          <p className="text-ink-3">
            Your staff, who&apos;s working when, and time off.
          </p>
        </div>
        <div className="flex items-center gap-1">
          {(
            [
              ["people", "People"],
              ["schedule", "Schedule"],
              ["timeoff", "Time off"],
            ] as const
          ).map(([k, name]) => (
            <button
              key={k}
              onClick={() => setTab(k)}
              className={`rounded-lg px-3 py-1.5 text-sm font-semibold ${
                tab === k
                  ? "bg-accent-soft text-accent-2"
                  : "text-ink-3 hover:bg-surface-2"
              }`}
            >
              {name}
            </button>
          ))}
        </div>
      </div>

      {error && (
        <Card className="p-4 text-sm text-bad">
          <p role="alert">{error}</p>
        </Card>
      )}

      {tab === "people" && (
        <People
          roster={roster}
          loaded={loaded}
          manage={manage}
          seePay={can("payroll:read")}
          onChanged={loadRoster}
        />
      )}
      {tab === "schedule" && <Schedule roster={roster} manage={manage} />}
      {tab === "timeoff" && <TimeOffTab manage={manage} />}
    </div>
  );
}

// ---- people -----------------------------------------------------------------

function People({
  roster,
  loaded,
  manage,
  seePay,
  onChanged,
}: {
  roster: Employee[];
  loaded: boolean;
  manage: boolean;
  seePay: boolean;
  onChanged: () => void;
}) {
  const [editing, setEditing] = useState<Employee | null>(null);
  const cols = seePay
    ? "grid-cols-[1.6fr_1fr_1fr_1.2fr_.7fr_.7fr_.7fr]"
    : "grid-cols-[1.6fr_1fr_1fr_1.2fr_.7fr]";
  const sorted = [...roster].sort(
    (a, b) =>
      Number(!a.profile) - Number(!b.profile) ||
      Number(!(a.profile?.current ?? true)) -
        Number(!(b.profile?.current ?? true)) ||
      a.name.localeCompare(b.name)
  );

  return (
    <Card className="overflow-x-auto">
      <div className="min-w-[760px]">
        <div
          className={`grid ${cols} gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3`}
        >
          <span>Name</span>
          <span>Title</span>
          <span>Type</span>
          <span />
          <span className="text-right">This week</span>
          {seePay && <span className="text-right">Pay / h</span>}
          {seePay && <span className="text-right">Bill / h</span>}
        </div>
        <div className="divide-y divide-line">
          {sorted.map((p) => (
            <div
              key={p.user_id}
              role={p.profile && manage ? "button" : undefined}
              onClick={() => p.profile && manage && setEditing(p)}
              className={`grid ${cols} items-center gap-4 px-5 py-3.5 ${
                p.profile && manage ? "cursor-pointer hover:bg-surface-2" : ""
              }`}
            >
              <div className="flex min-w-0 items-center gap-3">
                <span
                  className="h-3 w-3 shrink-0 rounded-full"
                  style={{
                    backgroundColor:
                      p.profile?.calendar_color ?? "var(--line-2)",
                  }}
                />
                <div className="min-w-0">
                  <div className="truncate font-semibold">{p.name}</div>
                  <div className="truncate text-xs text-ink-3">{p.email}</div>
                </div>
              </div>
              <span className="truncate text-sm text-ink-2">
                {p.profile?.title ?? "—"}
              </span>
              <span className="text-sm text-ink-2">
                {p.profile ? EMPLOYMENT_LABELS[p.profile.employment_type] : "—"}
              </span>
              <span className="flex flex-wrap items-center gap-1.5">
                {p.clocked_in && <Badge tone="good">on the clock</Badge>}
                {!p.profile && <Badge tone="warn">not set up</Badge>}
                {p.profile && !p.profile.current && <Badge>ended</Badge>}
                {!p.profile && manage && (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      setEditing(p);
                    }}
                    className="rounded-lg border border-line px-2 py-1 text-xs font-semibold text-ink-2 hover:border-accent"
                  >
                    Set up
                  </button>
                )}
              </span>
              <span className="text-right text-sm font-semibold tabular-nums">
                {p.profile ? hm(p.week_minutes) : "—"}
              </span>
              {seePay && (
                <span className="text-right text-sm tabular-nums">
                  {p.profile?.pay_rate_cents != null
                    ? money(p.profile.pay_rate_cents)
                    : "—"}
                </span>
              )}
              {seePay && (
                <span className="text-right text-sm tabular-nums">
                  {p.profile?.bill_rate_cents != null
                    ? money(p.profile.bill_rate_cents)
                    : "—"}
                </span>
              )}
            </div>
          ))}
          {!loaded && (
            <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
          )}
          {loaded && roster.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No staff yet. Invite people from Members first.
            </div>
          )}
        </div>
      </div>

      <Dialog open={!!editing} onOpenChange={(o) => !o && setEditing(null)}>
        <DialogContent className="max-h-[90vh] max-w-2xl overflow-y-auto">
          {editing && (
            <ProfileForm
              key={editing.user_id}
              person={editing}
              seePay={seePay}
              onDone={() => {
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

function ProfileForm({
  person,
  seePay,
  onDone,
}: {
  person: Employee;
  seePay: boolean;
  onDone: () => void;
}) {
  const p = person.profile;
  const [title, setTitle] = useState(p?.title ?? "");
  const [employment, setEmployment] = useState<EmploymentType>(
    p?.employment_type ?? "full_time"
  );
  const [pay, setPay] = useState(dollars(p?.pay_rate_cents));
  const [bill, setBill] = useState(dollars(p?.bill_rate_cents));
  const [hire, setHire] = useState(p?.hire_date ?? "");
  const [endDate, setEndDate] = useState(p?.end_date ?? "");
  const [target, setTarget] = useState(String(p?.weekly_hours_target ?? 40));
  const [vehicle, setVehicle] = useState<"company" | "personal">(
    p?.default_vehicle ?? "company"
  );
  const [mileage, setMileage] = useState(p?.mileage_reimbursed ?? false);
  const [ecName, setEcName] = useState(p?.emergency_contact_name ?? "");
  const [ecPhone, setEcPhone] = useState(p?.emergency_contact_phone ?? "");
  const [color, setColor] = useState(p?.calendar_color ?? "#3b82f6");
  const [notes, setNotes] = useState(p?.notes ?? "");
  const [busy, setBusy] = useState(false);

  async function save() {
    const body: ProfileInput = {
      title: title.trim() || null,
      employment_type: employment,
      hire_date: hire || null,
      end_date: endDate || null,
      weekly_hours_target: Number(target) || 0,
      default_vehicle: vehicle,
      mileage_reimbursed: mileage,
      emergency_contact_name: ecName.trim() || null,
      emergency_contact_phone: ecPhone.trim() || null,
      calendar_color: color,
      notes: notes.trim() || null,
    };
    if (seePay) {
      body.pay_rate_cents = toCents(pay);
      body.bill_rate_cents = toCents(bill);
    }
    setBusy(true);
    try {
      await team.saveProfile(person.user_id, body);
      toast.success(p ? `Saved ${person.name}` : `${person.name} is set up`);
      onDone();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save the profile"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>{person.name}</DialogTitle>
        <DialogDescription>
          {p
            ? person.email
            : "Set up their employee profile so they can clock in."}
        </DialogDescription>
      </DialogHeader>
      <div className="grid gap-4 sm:grid-cols-2">
        <label className={label}>
          Title
          <input
            className={field}
            placeholder="e.g. Maintenance technician"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
        </label>
        <label className={label}>
          Employment type
          <select
            className={field}
            value={employment}
            onChange={(e) => setEmployment(e.target.value as EmploymentType)}
          >
            {(Object.keys(EMPLOYMENT_LABELS) as EmploymentType[]).map((k) => (
              <option key={k} value={k}>
                {EMPLOYMENT_LABELS[k]}
              </option>
            ))}
          </select>
        </label>
        {seePay && (
          <>
            <label className={label}>
              Pay rate ($ per hour)
              <input
                className={field}
                inputMode="decimal"
                placeholder="0.00"
                value={pay}
                onChange={(e) => setPay(e.target.value)}
              />
            </label>
            <label className={label}>
              Bill rate ($ per hour)
              <input
                className={field}
                inputMode="decimal"
                placeholder="0.00"
                value={bill}
                onChange={(e) => setBill(e.target.value)}
              />
              <span className="font-normal">
                What an hour of their work is charged to owners.
              </span>
            </label>
          </>
        )}
        <label className={label}>
          Hire date
          <input
            className={field}
            type="date"
            value={hire}
            onChange={(e) => setHire(e.target.value)}
          />
        </label>
        <label className={label}>
          End date
          <input
            className={field}
            type="date"
            value={endDate}
            onChange={(e) => setEndDate(e.target.value)}
          />
          <span className="font-normal">Leave empty while they work here.</span>
        </label>
        <label className={label}>
          Weekly hours target
          <input
            className={field}
            type="number"
            min={0}
            value={target}
            onChange={(e) => setTarget(e.target.value)}
          />
        </label>
        <label className={label}>
          Usually drives
          <select
            className={field}
            value={vehicle}
            onChange={(e) =>
              setVehicle(e.target.value as "company" | "personal")
            }
          >
            <option value="company">A company vehicle</option>
            <option value="personal">Their own vehicle</option>
          </select>
        </label>
        <label className="flex items-center gap-2 text-sm sm:col-span-2">
          <input
            type="checkbox"
            checked={mileage}
            onChange={(e) => setMileage(e.target.checked)}
          />
          Pay back miles driven in their own vehicle
        </label>
        <label className={label}>
          Emergency contact
          <input
            className={field}
            placeholder="Name"
            value={ecName}
            onChange={(e) => setEcName(e.target.value)}
          />
        </label>
        <label className={label}>
          Emergency phone
          <input
            className={field}
            type="tel"
            value={ecPhone}
            onChange={(e) => setEcPhone(e.target.value)}
          />
        </label>
        <label className={label}>
          Calendar color
          <input
            className="h-10 w-20 cursor-pointer rounded-xl border border-line bg-surface"
            type="color"
            value={color}
            onChange={(e) => setColor(e.target.value)}
          />
        </label>
        <label className={`${label} sm:col-span-2`}>
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
          {busy ? "Saving…" : p ? "Save" : "Set up"}
        </Button>
      </DialogFooter>
    </>
  );
}

// ---- schedule ---------------------------------------------------------------

const SHIFT_KIND_LABELS: Record<Shift["kind"], string> = {
  work: "Work",
  on_call: "On call",
  training: "Training",
};

type ShiftDraft = { shift: Shift | null; userId?: string; date?: string };

function Schedule({ roster, manage }: { roster: Employee[]; manage: boolean }) {
  const [week, setWeek] = useState(() => weekStart(new Date()));
  const [shifts, setShifts] = useState<Shift[]>([]);
  const [off, setOff] = useState<TimeOff[]>([]);
  const [draft, setDraft] = useState<ShiftDraft | null>(null);
  const days = useMemo(
    () => Array.from({ length: 7 }, (_, i) => addDays(week, i)),
    [week]
  );
  const from = isoDate(week);
  const to = isoDate(addDays(week, 6));
  const today = useMemo(() => isoDate(new Date()), []);

  const load = useCallback(() => {
    team
      .shifts({ from, to })
      .then(setShifts)
      .catch((e) => toast.error(errMsg(e, "Couldn't load the schedule")));
    team
      .timeOff("approved")
      .then((list) =>
        setOff(list.filter((t) => t.starts_on <= to && t.ends_on >= from))
      )
      .catch(() => undefined);
  }, [from, to]);

  useEffect(() => {
    load();
  }, [load]);

  const people = roster.filter((p) => p.profile && p.profile.current);
  const colorOf = (id: string) =>
    roster.find((p) => p.user_id === id)?.profile?.calendar_color ?? "#64748b";
  // Anyone with a shift but no current profile still gets a row.
  const extra = shifts
    .filter((s) => !people.some((p) => p.user_id === s.user_id))
    .reduce<
      { user_id: string; name: string }[]
    >((acc, s) => (acc.some((a) => a.user_id === s.user_id) ? acc : [...acc, { user_id: s.user_id, name: s.user_name }]), []);
  const rows = [
    ...people.map((p) => ({ user_id: p.user_id, name: p.name })),
    ...extra,
  ];

  return (
    <Card className="overflow-hidden">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-4">
        <div>
          <h2 className="font-display text-lg font-bold">
            Week of{" "}
            {week.toLocaleDateString([], {
              month: "long",
              day: "numeric",
            })}
          </h2>
          <p className="text-sm text-ink-3">
            {shifts.length} shift{shifts.length === 1 ? "" : "s"} ·{" "}
            {hm(shifts.reduce((s, x) => s + x.minutes, 0))} hours scheduled
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
          {manage && (
            <Button onClick={() => setDraft({ shift: null })}>Add shift</Button>
          )}
        </div>
      </div>
      <div className="overflow-x-auto">
        <div className="min-w-[900px]">
          <div className="grid grid-cols-[160px_repeat(7,1fr)] border-b border-line text-xs font-bold uppercase tracking-wide text-ink-3">
            <div className="px-4 py-3">Person</div>
            {days.map((d) => (
              <div
                key={d.toISOString()}
                className={`px-2 py-3 ${isoDate(d) === today ? "text-accent-2" : ""}`}
              >
                {d.toLocaleDateString([], {
                  weekday: "short",
                  day: "numeric",
                })}
              </div>
            ))}
          </div>
          <div className="divide-y divide-line">
            {rows.map((r) => {
              const color = colorOf(r.user_id);
              return (
                <div
                  key={r.user_id}
                  className="grid grid-cols-[160px_repeat(7,1fr)]"
                >
                  <div className="flex items-center gap-2 px-4 py-3 text-sm font-semibold">
                    <span
                      className="h-2.5 w-2.5 shrink-0 rounded-full"
                      style={{ backgroundColor: color }}
                    />
                    <span className="truncate">{r.name}</span>
                  </div>
                  {days.map((d) => {
                    const ymd = isoDate(d);
                    const dayShifts = shifts.filter(
                      (s) =>
                        s.user_id === r.user_id &&
                        isoDate(new Date(s.starts_at)) === ymd
                    );
                    const away = off.find(
                      (t) =>
                        t.user_id === r.user_id &&
                        t.starts_on <= ymd &&
                        t.ends_on >= ymd
                    );
                    return (
                      <div
                        key={ymd}
                        onClick={() =>
                          manage &&
                          setDraft({
                            shift: null,
                            userId: r.user_id,
                            date: ymd,
                          })
                        }
                        className={`min-h-16 space-y-1 border-l border-line p-1.5 ${
                          manage ? "cursor-pointer hover:bg-surface-2" : ""
                        } ${ymd === today ? "bg-accent-soft/30" : ""}`}
                      >
                        {away && (
                          <div className="rounded-lg bg-surface-2 px-2 py-1 text-xs text-ink-3">
                            Off · {away.kind}
                          </div>
                        )}
                        {dayShifts.map((s) => (
                          <button
                            key={s.id}
                            onClick={(e) => {
                              e.stopPropagation();
                              if (manage) setDraft({ shift: s });
                            }}
                            title={s.notes ?? undefined}
                            className="block w-full rounded-lg px-2 py-1 text-left text-xs"
                            style={{
                              backgroundColor: `color-mix(in srgb, ${color} 18%, transparent)`,
                              borderLeft: `3px solid ${color}`,
                            }}
                          >
                            <div className="font-semibold text-ink">
                              {fmtTime(s.starts_at)}–{fmtTime(s.ends_at)}
                            </div>
                            {s.kind !== "work" && (
                              <div className="text-ink-2">
                                {SHIFT_KIND_LABELS[s.kind]}
                              </div>
                            )}
                          </button>
                        ))}
                      </div>
                    );
                  })}
                </div>
              );
            })}
            {rows.length === 0 && (
              <div className="px-5 py-10 text-center text-ink-3">
                Set up employee profiles on the People tab to build a schedule.
              </div>
            )}
          </div>
        </div>
      </div>

      <Dialog open={!!draft} onOpenChange={(o) => !o && setDraft(null)}>
        <DialogContent>
          {draft && (
            <ShiftForm
              key={draft.shift?.id ?? `${draft.userId}-${draft.date}`}
              draft={draft}
              people={people}
              defaultDate={draft.date ?? isoDate(days[0])}
              onDone={() => {
                setDraft(null);
                load();
              }}
            />
          )}
        </DialogContent>
      </Dialog>
    </Card>
  );
}

function ShiftForm({
  draft,
  people,
  defaultDate,
  onDone,
}: {
  draft: ShiftDraft;
  people: Employee[];
  defaultDate: string;
  onDone: () => void;
}) {
  const s = draft.shift;
  const [userId, setUserId] = useState(
    s?.user_id ?? draft.userId ?? people[0]?.user_id ?? ""
  );
  const [date, setDate] = useState(
    s ? isoDate(new Date(s.starts_at)) : defaultDate
  );
  const [start, setStart] = useState(s ? hhmm(s.starts_at) : "08:00");
  const [end, setEnd] = useState(s ? hhmm(s.ends_at) : "16:30");
  const [kind, setKind] = useState<Shift["kind"]>(s?.kind ?? "work");
  const [notes, setNotes] = useState(s?.notes ?? "");
  const [busy, setBusy] = useState(false);

  async function save() {
    if (!userId || !date || !start || !end) {
      toast.error("Pick a person, a day and the hours");
      return;
    }
    // A finish earlier than the start runs past midnight.
    const [y, m, d] = date.split("-").map(Number);
    const endDate = end <= start ? isoDate(new Date(y, m - 1, d + 1)) : date;
    const body: ShiftInput = {
      user_id: userId,
      starts_at: localToRfc3339(`${date}T${start}`),
      ends_at: localToRfc3339(`${endDate}T${end}`),
      kind,
      property_id: s?.property_id ?? null,
      notes: notes.trim() || null,
    };
    setBusy(true);
    try {
      if (s) await team.editShift(s.id, body);
      else await team.addShift(body);
      toast.success(s ? "Shift updated" : "Shift added");
      onDone();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save the shift"));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!s || !confirm("Delete this shift?")) return;
    setBusy(true);
    try {
      await team.deleteShift(s.id);
      toast.success("Shift deleted");
      onDone();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't delete the shift"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>{s ? "Change shift" : "Add shift"}</DialogTitle>
        <DialogDescription>
          {s ? s.user_name : "Put someone on the schedule."}
        </DialogDescription>
      </DialogHeader>
      <div className="grid gap-4 sm:grid-cols-2">
        <label className={`${label} sm:col-span-2`}>
          Person
          <select
            className={field}
            value={userId}
            onChange={(e) => setUserId(e.target.value)}
          >
            {!people.some((p) => p.user_id === userId) && s && (
              <option value={s.user_id}>{s.user_name}</option>
            )}
            {people.map((p) => (
              <option key={p.user_id} value={p.user_id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <label className={label}>
          Day
          <input
            className={field}
            type="date"
            value={date}
            onChange={(e) => setDate(e.target.value)}
          />
        </label>
        <label className={label}>
          Kind
          <select
            className={field}
            value={kind}
            onChange={(e) => setKind(e.target.value as Shift["kind"])}
          >
            {(Object.keys(SHIFT_KIND_LABELS) as Shift["kind"][]).map((k) => (
              <option key={k} value={k}>
                {SHIFT_KIND_LABELS[k]}
              </option>
            ))}
          </select>
        </label>
        <label className={label}>
          Starts
          <input
            className={field}
            type="time"
            value={start}
            onChange={(e) => setStart(e.target.value)}
          />
        </label>
        <label className={label}>
          Ends
          <input
            className={field}
            type="time"
            value={end}
            onChange={(e) => setEnd(e.target.value)}
          />
          {end && start && end <= start && (
            <span className="font-normal">Ends the next day.</span>
          )}
        </label>
        <label className={`${label} sm:col-span-2`}>
          Notes
          <input
            className={field}
            placeholder="e.g. Turnover at Maple Court"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
          />
        </label>
      </div>
      <DialogFooter>
        {s && (
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
          {busy ? "Saving…" : s ? "Save" : "Add shift"}
        </Button>
      </DialogFooter>
    </>
  );
}

// ---- time off ---------------------------------------------------------------

const TIME_OFF_TONE: Record<
  TimeOff["status"],
  "warn" | "good" | "bad" | "neutral"
> = {
  pending: "warn",
  approved: "good",
  denied: "bad",
  cancelled: "neutral",
};

const STATUS_ORDER: Record<TimeOff["status"], number> = {
  pending: 0,
  approved: 1,
  denied: 2,
  cancelled: 3,
};

function TimeOffTab({ manage }: { manage: boolean }) {
  const [list, setList] = useState<TimeOff[]>([]);
  const [loaded, setLoaded] = useState(false);

  const load = useCallback(() => {
    team
      .timeOff()
      .then((l) => {
        setList(l);
        setLoaded(true);
      })
      .catch((e) => toast.error(errMsg(e, "Couldn't load time off")));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const sorted = [...list].sort(
    (a, b) =>
      STATUS_ORDER[a.status] - STATUS_ORDER[b.status] ||
      (a.status === "pending"
        ? a.starts_on.localeCompare(b.starts_on)
        : b.starts_on.localeCompare(a.starts_on))
  );

  return (
    <Card className="overflow-hidden">
      <div className="divide-y divide-line">
        {sorted.map((r) => (
          <TimeOffRow key={r.id} r={r} manage={manage} onChanged={load} />
        ))}
        {!loaded && (
          <div className="px-5 py-10 text-center text-ink-3">Loading…</div>
        )}
        {loaded && list.length === 0 && (
          <div className="px-5 py-10 text-center text-ink-3">
            No time off requests.
          </div>
        )}
      </div>
    </Card>
  );
}

function TimeOffRow({
  r,
  manage,
  onChanged,
}: {
  r: TimeOff;
  manage: boolean;
  onChanged: () => void;
}) {
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);

  async function review(approve: boolean) {
    setBusy(true);
    try {
      await team.reviewTimeOff(r.id, approve, note.trim() || undefined);
      toast.success(
        approve
          ? `Approved ${r.user_name}'s time off`
          : `Denied ${r.user_name}'s time off`
      );
      onChanged();
    } catch (e) {
      toast.error(errMsg(e, "Couldn't save that"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-wrap items-center gap-4 px-5 py-4">
      <div className="min-w-56 flex-1">
        <div className="font-semibold">{r.user_name}</div>
        <div className="text-sm text-ink-2">
          {fmtDate(r.starts_on)}
          {r.ends_on !== r.starts_on && <> – {fmtDate(r.ends_on)}</>} · {r.days}{" "}
          day{r.days === 1 ? "" : "s"} · {r.kind}
        </div>
        {r.reason && <div className="text-sm text-ink-3">“{r.reason}”</div>}
        {r.review_note && (
          <div className="text-xs text-ink-3">Note: {r.review_note}</div>
        )}
      </div>
      <Badge tone={TIME_OFF_TONE[r.status]}>{r.status}</Badge>
      {manage && r.status === "pending" && (
        <div className="flex flex-wrap items-center gap-2">
          <input
            className={`${field} w-48`}
            placeholder="Note (optional)"
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
          <Button onClick={() => review(true)} disabled={busy}>
            Approve
          </Button>
          <Button
            variant="outline"
            onClick={() => review(false)}
            disabled={busy}
          >
            Deny
          </Button>
        </div>
      )}
    </div>
  );
}
