"use client";

// The weekly schedule: one row per person with a current profile, a column
// per day, approved time off shown in place. Managers press an empty day to
// add a shift or a shift to change it.

import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarPlus, ChevronLeft, ChevronRight, Trash2 } from "lucide-react";
import { toast } from "sonner";
import {
  hm,
  isoDate,
  localToRfc3339,
  team,
  weekStart,
  type Employee,
  type Shift,
  type ShiftInput,
} from "@/lib/backoffice";
import { Button } from "@/components/ui/button";
import { Field, Input, fieldClass } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/menu";
import { Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const KIND_LABELS: Record<Shift["kind"], string> = {
  work: "Work",
  on_call: "On call",
  training: "Training",
};

type Draft = { shift: Shift | null; userId?: string; date?: string };

function addDays(d: Date, n: number) {
  const x = new Date(d);
  x.setDate(x.getDate() + n);
  return x;
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

export function Schedule({
  roster,
  manage,
}: {
  roster: Employee[];
  manage: boolean;
}) {
  const [week, setWeek] = useState(() => weekStart(new Date()));
  const [draft, setDraft] = useState<Draft | null>(null);
  const days = useMemo(
    () => Array.from({ length: 7 }, (_, i) => addDays(week, i)),
    [week]
  );
  const from = isoDate(week);
  const to = isoDate(addDays(week, 6));
  const today = isoDate(new Date());

  const shiftsQ = useQuery({
    queryKey: ["team", "shifts", from, to],
    queryFn: () => team.shifts({ from, to }),
  });
  const offQ = useQuery({
    queryKey: ["team", "time-off", "approved"],
    queryFn: () => team.timeOff("approved"),
  });
  const shifts = useMemo(() => shiftsQ.data ?? [], [shiftsQ.data]);
  const off = (offQ.data ?? []).filter(
    (t) => t.starts_on <= to && t.ends_on >= from
  );

  const people = roster.filter((p) => p.profile && p.profile.current);
  const colorOf = (id: string) =>
    roster.find((p) => p.user_id === id)?.profile?.calendar_color ?? "#64748b";
  // Anyone with a shift but no current profile still gets a row.
  const rows = useMemo(() => {
    const base = people.map((p) => ({ user_id: p.user_id, name: p.name }));
    for (const s of shifts) {
      if (!base.some((r) => r.user_id === s.user_id))
        base.push({ user_id: s.user_id, name: s.user_name });
    }
    return base;
  }, [people, shifts]);

  return (
    <Panel className="overflow-hidden">
      <div className="flex flex-col gap-3 border-b border-line px-5 py-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h2 className="text-[15px] font-semibold text-fg">
            Week of{" "}
            {week.toLocaleDateString([], { month: "long", day: "numeric" })}
          </h2>
          <p className="mt-0.5 text-[13px] text-fg-3">
            {shifts.length} shift{shifts.length === 1 ? "" : "s"} ·{" "}
            {hm(shifts.reduce((s, x) => s + x.minutes, 0))} scheduled
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
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
          {manage && (
            <Button size="sm" onClick={() => setDraft({ shift: null })}>
              <CalendarPlus />
              Add shift
            </Button>
          )}
        </div>
      </div>
      {shiftsQ.error && (
        <p className="px-5 py-3 text-[13px] text-bad">
          Couldn&apos;t load the schedule: {shiftsQ.error.message}
        </p>
      )}
      <div className="overflow-x-auto">
        <div className="min-w-[900px]">
          <div className="grid grid-cols-[160px_repeat(7,1fr)] border-b border-line">
            <div className="eyebrow px-4 py-2.5">Person</div>
            {days.map((d) => (
              <div
                key={d.toISOString()}
                className={cn(
                  "eyebrow px-2 py-2.5",
                  isoDate(d) === today && "text-accent"
                )}
              >
                {d.toLocaleDateString([], { weekday: "short", day: "numeric" })}
              </div>
            ))}
          </div>
          {shiftsQ.isLoading && (
            <div className="space-y-2 p-3">
              <Skeleton className="h-16" />
              <Skeleton className="h-16" />
            </div>
          )}
          <div className="divide-y divide-line">
            {rows.map((r) => {
              const color = colorOf(r.user_id);
              return (
                <div
                  key={r.user_id}
                  className="grid grid-cols-[160px_repeat(7,1fr)]"
                >
                  <div className="flex items-center gap-2 px-4 py-3 text-[13px] font-medium text-fg">
                    <span
                      aria-hidden
                      className="size-2.5 shrink-0 rounded-full"
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
                        className={cn(
                          "relative min-h-16 space-y-1 border-l border-line p-1.5",
                          ymd === today && "bg-accent/5"
                        )}
                      >
                        {manage && (
                          <button
                            type="button"
                            aria-label={`Add a shift for ${r.name} on ${ymd}`}
                            onClick={() =>
                              setDraft({
                                shift: null,
                                userId: r.user_id,
                                date: ymd,
                              })
                            }
                            className="absolute inset-0 transition hover:bg-fill-2"
                          />
                        )}
                        {away && (
                          <div className="relative rounded-lg bg-fill px-2 py-1 text-xs text-fg-3">
                            Off · {away.kind}
                          </div>
                        )}
                        {dayShifts.map((s) => (
                          <button
                            key={s.id}
                            type="button"
                            disabled={!manage}
                            onClick={() => setDraft({ shift: s })}
                            title={s.notes ?? undefined}
                            className="relative block w-full rounded-lg px-2 py-1 text-left text-xs disabled:cursor-default"
                            style={{
                              backgroundColor: `color-mix(in srgb, ${color} 18%, transparent)`,
                              borderLeft: `3px solid ${color}`,
                            }}
                          >
                            <div className="font-medium text-fg">
                              {fmtTime(s.starts_at)} to {fmtTime(s.ends_at)}
                            </div>
                            {s.kind !== "work" && (
                              <div className="text-fg-2">
                                {KIND_LABELS[s.kind]}
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
            {!shiftsQ.isLoading && rows.length === 0 && (
              <div className="px-5 py-10 text-center text-[13px] text-fg-3">
                Set up employee profiles on the People tab to build a schedule.
              </div>
            )}
          </div>
        </div>
      </div>

      {draft && (
        <ShiftDialog
          key={draft.shift?.id ?? `${draft.userId}-${draft.date}`}
          draft={draft}
          people={people}
          defaultDate={draft.date ?? isoDate(days[0])}
          onClose={() => setDraft(null)}
        />
      )}
    </Panel>
  );
}

function ShiftDialog({
  draft,
  people,
  defaultDate,
  onClose,
}: {
  draft: Draft;
  people: Employee[];
  defaultDate: string;
  onClose: () => void;
}) {
  const qc = useQueryClient();
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

  async function done(msg: string) {
    toast.success(msg);
    await qc.invalidateQueries({ queryKey: ["team"] });
    onClose();
  }

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
      await done(s ? "Shift updated" : "Shift added");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save the shift");
      setBusy(false);
    }
  }

  async function remove() {
    if (!s || !window.confirm("Delete this shift?")) return;
    setBusy(true);
    try {
      await team.deleteShift(s.id);
      await done("Shift deleted");
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't delete the shift");
      setBusy(false);
    }
  }

  const select = cn(fieldClass, "h-11 w-full");

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md">
        <DialogTitle className="text-[17px] font-semibold">
          {s ? "Change shift" : "Add shift"}
        </DialogTitle>
        <DialogDescription className="mt-1 text-[13px] text-fg-3">
          {s ? s.user_name : "Put someone on the schedule."}
        </DialogDescription>
        <form
          className="mt-4"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="sm:col-span-2">
              <Field label="Person">
                {(f) => (
                  <select
                    {...f}
                    className={select}
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
                )}
              </Field>
            </div>
            <Field label="Day">
              {(f) => (
                <Input
                  {...f}
                  type="date"
                  value={date}
                  onChange={(e) => setDate(e.target.value)}
                />
              )}
            </Field>
            <Field label="Kind">
              {(f) => (
                <select
                  {...f}
                  className={select}
                  value={kind}
                  onChange={(e) => setKind(e.target.value as Shift["kind"])}
                >
                  {(Object.keys(KIND_LABELS) as Shift["kind"][]).map((k) => (
                    <option key={k} value={k}>
                      {KIND_LABELS[k]}
                    </option>
                  ))}
                </select>
              )}
            </Field>
            <Field label="Starts">
              {(f) => (
                <Input
                  {...f}
                  type="time"
                  value={start}
                  onChange={(e) => setStart(e.target.value)}
                />
              )}
            </Field>
            <Field
              label="Ends"
              hint={
                end && start && end <= start ? "Ends the next day." : undefined
              }
            >
              {(f) => (
                <Input
                  {...f}
                  type="time"
                  value={end}
                  onChange={(e) => setEnd(e.target.value)}
                />
              )}
            </Field>
            <div className="sm:col-span-2">
              <Field label="Notes">
                {(f) => (
                  <Input
                    {...f}
                    placeholder="e.g. Turnover at Maple Court"
                    value={notes}
                    onChange={(e) => setNotes(e.target.value)}
                  />
                )}
              </Field>
            </div>
          </div>
          <div className="mt-5 flex items-center gap-2">
            {s && (
              <Button
                type="button"
                variant="danger"
                size="sm"
                disabled={busy}
                onClick={remove}
              >
                <Trash2 />
                Delete
              </Button>
            )}
            <div className="ml-auto flex gap-2">
              <Button type="button" variant="ghost" onClick={onClose}>
                Cancel
              </Button>
              <Button type="submit" loading={busy}>
                {s ? "Save" : "Add shift"}
              </Button>
            </div>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
