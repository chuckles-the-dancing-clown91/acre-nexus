"use client";

// Reminders calendar (#54): one schedule for everything with a due date —
// lease renewals, license/insurance expirations, tours, and inspections.
// A hand-rolled month grid plus an "Upcoming" side list with quick actions.

import { useMemo, useState } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";

import { useAuth } from "@/lib/auth";
import {
  useCreateReminder,
  useDeleteReminder,
  useReminders,
  useUpdateReminder,
} from "@/lib/queries";
import type { Reminder } from "@/lib/api";
import {
  createReminderSchema,
  REMINDER_SUBJECTS,
  type CreateReminderInputForm,
} from "@/lib/schemas";
import { clsx } from "@/lib/clsx";
import { Badge, Card } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";

const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/** Dot colour per subject type — aligned with the Badge tone palette. */
const SUBJECT_DOT: Record<string, string> = {
  lease: "bg-accent",
  license: "bg-warn",
  insurance: "bg-bad",
  tour: "bg-info",
  inspection: "bg-good",
  custom: "bg-ink-3",
};

function subjectTone(subject: string) {
  switch (subject) {
    case "lease":
      return "accent" as const;
    case "license":
      return "warn" as const;
    case "insurance":
      return "bad" as const;
    case "tour":
      return "info" as const;
    case "inspection":
      return "good" as const;
    default:
      return "neutral" as const;
  }
}

/** `YYYY-MM-DD` from local calendar parts (month is 0-based). */
function ymd(year: number, month: number, day: number) {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${year}-${pad(month + 1)}-${pad(day)}`;
}

/** "in 3 days" / "today" / "2 days overdue". */
function dueLabel(daysLeft: number) {
  if (daysLeft === 0) return "today";
  const n = Math.abs(daysLeft);
  const unit = n === 1 ? "day" : "days";
  return daysLeft > 0 ? `in ${n} ${unit}` : `${n} ${unit} overdue`;
}

export default function CalendarPage() {
  const { can } = useAuth();
  const canManage = can("calendar:manage");

  // First day of the displayed month.
  const [month, setMonth] = useState(() => {
    const now = new Date();
    return new Date(now.getFullYear(), now.getMonth(), 1);
  });

  const year = month.getFullYear();
  const monthIdx = month.getMonth();
  const daysInMonth = new Date(year, monthIdx + 1, 0).getDate();
  const from = ymd(year, monthIdx, 1);
  const to = ymd(year, monthIdx, daysInMonth);

  const monthQ = useReminders({ from, to });
  const activeQ = useReminders({ status: "active" });
  const updateReminder = useUpdateReminder();
  const deleteReminder = useDeleteReminder();

  const byDay = useMemo(() => {
    const map = new Map<string, Reminder[]>();
    for (const r of monthQ.data ?? []) {
      const list = map.get(r.due_date) ?? [];
      list.push(r);
      map.set(r.due_date, list);
    }
    return map;
  }, [monthQ.data]);

  const upcoming = useMemo(
    () =>
      (activeQ.data ?? [])
        .filter((r) => r.days_left != null && r.days_left >= 0)
        .sort((a, b) => (a.days_left ?? 0) - (b.days_left ?? 0))
        .slice(0, 8),
    [activeQ.data]
  );

  if (!can("calendar:read")) {
    return (
      <Card className="px-5 py-10 text-center text-ink-3">
        You don&apos;t have permission to view the calendar.
      </Card>
    );
  }

  const now = new Date();
  const today = ymd(now.getFullYear(), now.getMonth(), now.getDate());
  const firstWeekday = new Date(year, monthIdx, 1).getDay(); // 0 = Sunday
  const trailing = (7 - ((firstWeekday + daysInMonth) % 7)) % 7;
  const monthLabel = month.toLocaleString("en-US", {
    month: "long",
    year: "numeric",
  });
  const error = monthQ.error ?? activeQ.error;

  return (
    <div className="space-y-6">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Calendar &amp; reminders
          </h1>
          <p className="text-ink-3">
            One schedule for lease renewals, license and insurance expirations,
            tours, and inspections.
          </p>
        </div>
        {canManage && <NewReminderDialog />}
      </div>

      {error && <p className="text-bad">{error.message}</p>}

      <div className="grid gap-6 lg:grid-cols-[2fr_1fr]">
        <Card className="overflow-hidden">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-line px-5 py-4">
            <h2 className="font-display text-xl font-bold">{monthLabel}</h2>
            <div className="flex items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                aria-label="Previous month"
                onClick={() => setMonth(new Date(year, monthIdx - 1, 1))}
              >
                ←
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() =>
                  setMonth(new Date(now.getFullYear(), now.getMonth(), 1))
                }
              >
                Today
              </Button>
              <Button
                variant="outline"
                size="sm"
                aria-label="Next month"
                onClick={() => setMonth(new Date(year, monthIdx + 1, 1))}
              >
                →
              </Button>
            </div>
          </div>

          {monthQ.isLoading && (
            <div className="border-b border-line px-5 py-2 text-sm text-ink-3">
              Loading reminders…
            </div>
          )}

          <div className="grid grid-cols-7 border-b border-line text-center text-xs font-bold uppercase tracking-wide text-ink-3">
            {WEEKDAYS.map((d) => (
              <div key={d} className="py-2">
                {d}
              </div>
            ))}
          </div>

          <div className="grid grid-cols-7 gap-px bg-line">
            {Array.from({ length: firstWeekday }, (_, i) => (
              <div
                key={`lead-${i}`}
                className="min-h-[6.5rem] bg-surface-2/50"
              />
            ))}
            {Array.from({ length: daysInMonth }, (_, i) => {
              const day = i + 1;
              const date = ymd(year, monthIdx, day);
              const items = byDay.get(date) ?? [];
              const isToday = date === today;
              return (
                <div
                  key={date}
                  className="min-h-[6.5rem] overflow-hidden bg-surface p-1.5"
                >
                  <div
                    className={clsx(
                      "mb-1 flex h-6 w-6 items-center justify-center rounded-full text-xs font-bold",
                      isToday ? "bg-accent text-on-accent" : "text-ink-2"
                    )}
                  >
                    {day}
                  </div>
                  <div className="space-y-0.5">
                    {items.slice(0, 3).map((r) => (
                      <div
                        key={r.id}
                        title={`${r.title} — ${r.subject_type} (${r.status})`}
                        className="flex items-center gap-1 text-xs"
                      >
                        <span
                          className={clsx(
                            "h-1.5 w-1.5 shrink-0 rounded-full",
                            SUBJECT_DOT[r.subject_type] ?? SUBJECT_DOT.custom
                          )}
                        />
                        <span
                          className={clsx(
                            "truncate",
                            r.status === "done" && "text-ink-3 line-through",
                            r.status === "cancelled" &&
                              "text-ink-3 line-through opacity-60"
                          )}
                        >
                          {r.status === "done" ? "✓ " : ""}
                          {r.title}
                        </span>
                      </div>
                    ))}
                    {items.length > 3 && (
                      <div className="text-xs text-ink-3">
                        +{items.length - 3} more
                      </div>
                    )}
                  </div>
                </div>
              );
            })}
            {Array.from({ length: trailing }, (_, i) => (
              <div
                key={`trail-${i}`}
                className="min-h-[6.5rem] bg-surface-2/50"
              />
            ))}
          </div>

          {monthQ.data && monthQ.data.length === 0 && !monthQ.isLoading && (
            <div className="border-t border-line px-5 py-3 text-center text-sm text-ink-3">
              No reminders this month.
            </div>
          )}

          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 border-t border-line px-5 py-3 text-xs text-ink-3">
            {REMINDER_SUBJECTS.map((s) => (
              <span key={s} className="flex items-center gap-1.5 capitalize">
                <span
                  className={clsx("h-2 w-2 rounded-full", SUBJECT_DOT[s])}
                />
                {s}
              </span>
            ))}
          </div>
        </Card>

        <Card className="self-start overflow-hidden">
          <div className="border-b border-line px-5 py-4">
            <h2 className="font-display text-lg font-bold">Upcoming</h2>
            <p className="text-sm text-ink-3">
              Active reminders, soonest first.
            </p>
          </div>
          <div className="divide-y divide-line">
            {activeQ.isLoading && (
              <div className="px-5 py-8 text-center text-ink-3">Loading…</div>
            )}
            {upcoming.map((r) => (
              <div key={r.id} className="px-5 py-3.5">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="truncate font-semibold">{r.title}</div>
                    <div className="mt-1 flex flex-wrap items-center gap-2 text-sm">
                      <Badge
                        tone={subjectTone(r.subject_type)}
                        className="capitalize"
                      >
                        {r.subject_type}
                      </Badge>
                      <span className="text-ink-2">{r.due_date}</span>
                      {r.days_left != null && (
                        <span
                          className={clsx(
                            r.days_left < 0
                              ? "font-semibold text-bad"
                              : "text-ink-3"
                          )}
                        >
                          {dueLabel(r.days_left)}
                        </span>
                      )}
                    </div>
                    {r.lead_days.length > 0 && (
                      <div className="mt-1 text-xs text-ink-3">
                        reminds {r.lead_days.join("/")}d before
                      </div>
                    )}
                  </div>
                  {canManage && (
                    <div className="flex shrink-0 items-center gap-1">
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={updateReminder.isPending}
                        onClick={() =>
                          updateReminder.mutate({
                            id: r.id,
                            body: { status: "done" },
                          })
                        }
                      >
                        Done
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={updateReminder.isPending}
                        onClick={() =>
                          updateReminder.mutate({
                            id: r.id,
                            body: { status: "cancelled" },
                          })
                        }
                      >
                        Cancel
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="px-2 text-ink-3 hover:text-bad"
                        aria-label={`Delete ${r.title}`}
                        disabled={deleteReminder.isPending}
                        onClick={() => {
                          if (window.confirm(`Delete reminder “${r.title}”?`))
                            deleteReminder.mutate(r.id);
                        }}
                      >
                        ✕
                      </Button>
                    </div>
                  )}
                </div>
              </div>
            ))}
            {!activeQ.isLoading && upcoming.length === 0 && (
              <div className="px-5 py-8 text-center text-ink-3">
                Nothing coming up.
              </div>
            )}
          </div>
        </Card>
      </div>
    </div>
  );
}

/** Dialog to schedule a reminder (calendar:manage). */
function NewReminderDialog() {
  const [open, setOpen] = useState(false);
  const create = useCreateReminder();

  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<CreateReminderInputForm>({
    resolver: zodResolver(createReminderSchema),
    defaultValues: {
      subject_type: "custom",
      title: "",
      description: "",
      due_date: "",
      lead_days: "",
      recipients: "",
    },
  });

  const onSubmit = handleSubmit(async (values) => {
    const leadDays = values.lead_days
      ? values.lead_days
          .split(",")
          .map((s) => parseInt(s.trim(), 10))
          .filter((n) => Number.isFinite(n) && n >= 0)
      : [];
    await create.mutateAsync(
      {
        subject_type: values.subject_type,
        title: values.title,
        description: values.description || undefined,
        due_date: values.due_date,
        lead_days: leadDays.length > 0 ? leadDays : undefined,
        recipients: values.recipients
          ? values.recipients
              .split(",")
              .map((s) => s.trim())
              .filter((s) => s.includes("@"))
          : undefined,
      },
      {
        onSuccess: () => {
          reset();
          setOpen(false);
        },
      }
    );
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>New reminder</Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={onSubmit}>
          <DialogHeader>
            <DialogTitle>New reminder</DialogTitle>
            <DialogDescription>
              Schedule a due date with notify-ahead lead days.
            </DialogDescription>
          </DialogHeader>
          <div className="my-5 space-y-4">
            <div className="space-y-1.5">
              <Label>Subject type</Label>
              <select
                className="flex h-10 w-full rounded-xl border border-line bg-surface-2 px-3 text-sm capitalize outline-none focus:border-accent"
                aria-invalid={!!errors.subject_type}
                {...register("subject_type")}
              >
                {REMINDER_SUBJECTS.map((s) => (
                  <option key={s} value={s}>
                    {s}
                  </option>
                ))}
              </select>
              {errors.subject_type && (
                <p className="text-sm text-bad" role="alert">
                  {errors.subject_type.message}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>Title</Label>
              <Input
                placeholder="e.g. Business license renewal"
                aria-invalid={!!errors.title}
                {...register("title")}
              />
              {errors.title && (
                <p className="text-sm text-bad" role="alert">
                  {errors.title.message}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>Description (optional)</Label>
              <textarea
                rows={2}
                placeholder="Notes for whoever gets notified"
                className="flex w-full rounded-xl border border-line bg-surface-2 px-3 py-2.5 text-sm outline-none placeholder:text-ink-3 focus:border-accent"
                {...register("description")}
              />
            </div>
            <div className="space-y-1.5">
              <Label>Due date</Label>
              <Input
                type="date"
                aria-invalid={!!errors.due_date}
                {...register("due_date")}
              />
              {errors.due_date && (
                <p className="text-sm text-bad" role="alert">
                  {errors.due_date.message}
                </p>
              )}
            </div>
            <div className="space-y-1.5">
              <Label>Lead days (optional)</Label>
              <Input placeholder="30,7,1" {...register("lead_days")} />
              <p className="text-xs text-ink-3">
                Days before the due date to notify; blank = workspace default.
              </p>
            </div>
            <div className="space-y-1.5">
              <Label>Recipients (optional)</Label>
              <Input placeholder="email, email" {...register("recipients")} />
              <p className="text-xs text-ink-3">
                External emails; staff with calendar access are always notified.
              </p>
            </div>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setOpen(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={isSubmitting}>
              {isSubmitting ? "Scheduling…" : "Schedule reminder"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
