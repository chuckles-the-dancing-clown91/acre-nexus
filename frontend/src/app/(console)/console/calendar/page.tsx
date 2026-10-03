"use client";

// The week: every visit (repairs, showings, inspections) and every reminder
// due, by day. Filter to one person. Click through to the work order.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import {
  Bell,
  CalendarCheck,
  CalendarClock,
  CalendarX,
  ChevronLeft,
  ChevronRight,
  HardHat,
} from "lucide-react";
import { api } from "@/lib/api";
import {
  appointments,
  dayOf,
  statusWords,
  weekOf,
  ymd,
  type Appointment,
} from "@/lib/appointments";
import { useAuth } from "@/lib/auth";
import { desk } from "@/lib/servicedesk";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

export default function CalendarPage() {
  const { can, user } = useAuth();
  const [anchor, setAnchor] = useState(() => new Date());
  const [who, setWho] = useState<string>("");
  const week = useMemo(() => weekOf(anchor), [anchor]);
  const from = ymd(week[0]);
  const to = ymd(week[6]);
  const today = ymd(new Date());

  const visits = useQuery({
    queryKey: ["appointments", "week", from, who],
    queryFn: () => appointments.list({ from, to, assignee: who || undefined }),
    enabled: can("maintenance:read"),
  });
  const reminders = useQuery({
    queryKey: ["reminders", "week", from],
    queryFn: () => api.reminders({ from, to, status: "active" }),
    enabled: can("calendar:read") && !who,
  });
  const techs = useQuery({
    queryKey: ["techs", "all"],
    queryFn: () => desk.techs(),
    enabled: can("maintenance:read"),
  });

  const byDay = useMemo(() => {
    const m = new Map<string, Appointment[]>();
    for (const a of visits.data ?? []) {
      if (a.status === "cancelled") continue;
      const d = dayOf(a);
      if (!d) continue;
      m.set(d, [...(m.get(d) ?? []), a]);
    }
    for (const list of m.values())
      list.sort((x, y) =>
        (x.starts_at ?? x.windows[0]?.start ?? "").localeCompare(
          y.starts_at ?? y.windows[0]?.start ?? ""
        )
      );
    return m;
  }, [visits.data]);
  const dueRows = reminders.data;
  const remindersByDay = useMemo(() => {
    const m = new Map<string, NonNullable<typeof dueRows>>();
    for (const r of dueRows ?? [])
      m.set(r.due_date, [...(m.get(r.due_date) ?? []), r]);
    return m;
  }, [dueRows]);

  const shift = (days: number) => {
    const d = new Date(anchor);
    d.setDate(d.getDate() + days);
    setAnchor(d);
  };
  const label = `${week[0].toLocaleDateString(undefined, { month: "short", day: "numeric" })} to ${week[6].toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" })}`;
  const mine = (visits.data ?? []).filter(
    (a) => a.assignee_user_id === user?.id && a.status === "confirmed"
  ).length;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Calendar"
        title="This week"
        description="Visits with residents and prospects, and what's coming due."
        actions={
          <div className="flex items-center gap-2">
            <select
              aria-label="Whose visits"
              value={who}
              onChange={(e) => setWho(e.target.value)}
              className="rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg"
            >
              <option value="">Everyone</option>
              {user && <option value={user.id}>Mine</option>}
              {techs.data
                ?.filter((t) => t.user_id !== user?.id)
                .map((t) => (
                  <option key={t.user_id} value={t.user_id}>
                    {t.name}
                  </option>
                ))}
            </select>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => shift(-7)}
              aria-label="Previous week"
            >
              <ChevronLeft />
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => setAnchor(new Date())}
            >
              Today
            </Button>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => shift(7)}
              aria-label="Next week"
            >
              <ChevronRight />
            </Button>
          </div>
        }
      />
      <div className="flex items-center justify-between text-[13px] text-fg-3">
        <span>{label}</span>
        {mine > 0 && <span>{mine} of these are yours</span>}
      </div>

      {visits.isLoading && <Skeleton className="h-96" />}
      {visits.data && (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-7">
          {week.map((d) => {
            const key = ymd(d);
            const list = byDay.get(key) ?? [];
            const due = remindersByDay.get(key) ?? [];
            const isToday = key === today;
            return (
              <Panel
                key={key}
                className={cn(
                  "flex min-h-[12rem] flex-col p-3",
                  isToday && "border-accent/50"
                )}
              >
                <div className="mb-2 flex items-baseline justify-between">
                  <span
                    className={cn(
                      "text-xs font-medium",
                      isToday ? "text-accent" : "text-fg-3"
                    )}
                  >
                    {d.toLocaleDateString(undefined, { weekday: "short" })}
                  </span>
                  <span
                    className={cn(
                      "figure text-[15px] font-semibold",
                      isToday ? "text-accent" : "text-fg"
                    )}
                  >
                    {d.getDate()}
                  </span>
                </div>
                <ul className="space-y-1.5">
                  {list.map((a) => (
                    <li key={a.id}>
                      <VisitCard a={a} />
                    </li>
                  ))}
                  {due.map((r) => (
                    <li
                      key={r.id}
                      className="flex items-start gap-1.5 rounded-lg bg-fill/60 px-2 py-1.5 text-[12px] text-fg-2"
                    >
                      <Bell className="mt-0.5 size-3 shrink-0 text-fg-3" />
                      <span className="min-w-0 truncate" title={r.title}>
                        {r.title}
                      </span>
                    </li>
                  ))}
                </ul>
                {list.length === 0 && due.length === 0 && (
                  <span className="mt-auto text-[11px] text-fg-4">Nothing</span>
                )}
              </Panel>
            );
          })}
        </div>
      )}
    </div>
  );
}

function VisitCard({ a }: { a: Appointment }) {
  const href =
    a.subject_type === "ticket" && a.subject_id
      ? `/console/maintenance/${a.subject_id}`
      : `/console/properties/${a.property_id}`;
  const Icon =
    a.status === "confirmed"
      ? CalendarCheck
      : a.status === "declined"
        ? CalendarX
        : CalendarClock;
  // The server words the time in the workspace's zone, so staff anywhere
  // read the same clock.
  const time = a.when_words
    ? a.when_words.replace(/^[A-Za-z]{3}, [A-Za-z]{3} \d+, /, "")
    : a.windows.length
      ? `${a.windows.length} time${a.windows.length === 1 ? "" : "s"} offered`
      : "";
  return (
    <Link
      href={href}
      className={cn(
        "block rounded-lg border px-2 py-1.5 text-[12px] transition hover:bg-fill",
        a.status === "confirmed"
          ? "border-good/30 bg-good/[0.06]"
          : a.status === "declined"
            ? "border-warn/30 bg-warn/[0.06]"
            : "border-line"
      )}
      title={statusWords(a.status, a.with_role)}
    >
      <div className="flex items-center gap-1.5 font-medium text-fg">
        <Icon
          className={cn(
            "size-3.5 shrink-0",
            a.status === "confirmed"
              ? "text-good"
              : a.status === "declined"
                ? "text-warn"
                : "text-fg-3"
          )}
        />
        <span className="truncate">{time}</span>
      </div>
      <div className="mt-0.5 truncate text-fg-2">{a.title}</div>
      <div className="mt-0.5 flex items-center gap-1 truncate text-[11px] text-fg-3">
        {a.vendor_name ? <HardHat className="size-3" /> : null}
        {a.property_name}
        {a.assignee_name && ` · ${a.assignee_name}`}
      </div>
      {a.kind !== "repair" && <Badge className="mt-1">{a.kind}</Badge>}
    </Link>
  );
}
