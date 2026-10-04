"use client";

// A tech's day on a phone: the visits booked for today in order, the work
// orders with their tasks, and one big button to start and stop the clock on
// whichever job they're at. Time lands against the work order.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CalendarCheck,
  Check,
  ChevronRight,
  Clock,
  MapPin,
  Navigation,
  Phone,
  Play,
  Square,
} from "lucide-react";
import { toast } from "sonner";
import { appointments, ymd, type Appointment } from "@/lib/appointments";
import { useAuth } from "@/lib/auth";
import { currentLocation, hm, me, type TimeEntry } from "@/lib/backoffice";
import { desk, minutesLabel, type QueueTask } from "@/lib/servicedesk";
import { mapLink, phoneLinks } from "@/lib/showings";
import { ApiError } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

interface Job {
  ticket_id: string;
  title: string;
  property_id: string;
  property_name: string;
  location: string | null;
  priority: string;
  tasks: QueueTask[];
  visit: Appointment | null;
  minutes_today: number;
}

export default function MyDayPage() {
  const { user } = useAuth();
  const qc = useQueryClient();
  const today = ymd(new Date());
  const [busy, setBusy] = useState(false);

  const visits = useQuery({
    queryKey: ["appointments", "day", today, user?.id],
    queryFn: () =>
      appointments.list({ from: today, to: today, assignee: user?.id }),
    enabled: !!user,
  });
  const queue = useQuery({ queryKey: ["my-queue"], queryFn: desk.queue });
  // The clock is only for people on the team; everyone else just sees the day.
  const clock = useQuery({
    queryKey: ["me", "clock"],
    queryFn: me.clock,
    retry: false,
  });
  const time = useQuery({
    queryKey: ["me", "time", today],
    queryFn: () => me.time({ from: today, to: today }),
    retry: false,
  });
  const onTeam = !(
    clock.error instanceof ApiError && clock.error.status === 403
  );

  const jobs = useMemo<Job[]>(() => {
    const byTicket = new Map<string, Job>();
    const minutes = new Map<string, number>();
    for (const e of time.data ?? [])
      if (e.maintenance_ticket_id)
        minutes.set(
          e.maintenance_ticket_id,
          (minutes.get(e.maintenance_ticket_id) ?? 0) + e.minutes
        );
    for (const t of queue.data?.tasks ?? []) {
      const j = byTicket.get(t.ticket_id) ?? {
        ticket_id: t.ticket_id,
        title: t.ticket_title,
        property_id: t.property_id,
        property_name: t.property_name,
        location: t.location,
        priority: t.priority,
        tasks: [],
        visit: null,
        minutes_today: minutes.get(t.ticket_id) ?? 0,
      };
      j.tasks.push(t);
      byTicket.set(t.ticket_id, j);
    }
    for (const a of visits.data ?? []) {
      if (a.subject_type !== "ticket" || !a.subject_id) continue;
      if (a.status === "cancelled") continue;
      const j = byTicket.get(a.subject_id) ?? {
        ticket_id: a.subject_id,
        title: a.title,
        property_id: a.property_id,
        property_name: a.property_name ?? "",
        location: null,
        priority: "normal",
        tasks: [],
        visit: null,
        minutes_today: minutes.get(a.subject_id) ?? 0,
      };
      j.visit = a;
      byTicket.set(a.subject_id, j);
    }
    // Visits in time order first, then the rest of the queue in its order.
    return [...byTicket.values()].sort((x, y) => {
      const xs = x.visit?.starts_at ?? x.visit?.windows[0]?.start;
      const ys = y.visit?.starts_at ?? y.visit?.windows[0]?.start;
      if (xs && ys) return xs.localeCompare(ys);
      if (xs) return -1;
      if (ys) return 1;
      return 0;
    });
  }, [queue.data, visits.data, time.data]);

  const open: TimeEntry | null = clock.data?.open ?? null;
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["me"] });
    qc.invalidateQueries({ queryKey: ["my-queue"] });
    qc.invalidateQueries({ queryKey: ["appointments"] });
  };

  async function start(j: Job) {
    setBusy(true);
    try {
      const location = clock.data?.records_location
        ? ((await currentLocation()) ?? undefined)
        : undefined;
      await me.clockIn({
        kind: "work_order",
        maintenance_ticket_id: j.ticket_id,
        location,
      });
      toast.success(`On the clock at ${j.property_name}`);
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't start the clock");
    } finally {
      setBusy(false);
    }
  }
  async function stop() {
    setBusy(true);
    try {
      const location = clock.data?.records_location
        ? ((await currentLocation()) ?? undefined)
        : undefined;
      await me.clockOut({ location });
      toast.success("Off the clock");
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't stop the clock");
    } finally {
      setBusy(false);
    }
  }
  async function finishTask(t: QueueTask) {
    setBusy(true);
    try {
      await desk.updateTask(t.ticket_id, t.task_id, { status: "done" });
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't update it");
    } finally {
      setBusy(false);
    }
  }
  async function visitOutcome(a: Appointment, status: "done" | "no_show") {
    setBusy(true);
    try {
      await appointments.update(a.id, { status });
      toast.success(status === "done" ? "Visit done" : "Marked nobody home");
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't save that");
    } finally {
      setBusy(false);
    }
  }

  const loading = visits.isLoading || queue.isLoading;
  const dayLabel = new Date().toLocaleDateString(undefined, {
    weekday: "long",
    month: "short",
    day: "numeric",
  });

  return (
    <div className="mx-auto max-w-xl space-y-4">
      <PageHeader
        eyebrow="My day"
        title={dayLabel}
        description={
          jobs.length
            ? `${jobs.length} job${jobs.length === 1 ? "" : "s"}${
                visits.data?.length
                  ? `, ${visits.data.length} booked visit${visits.data.length === 1 ? "" : "s"}`
                  : ""
              }`
            : "Nothing on your list yet."
        }
      />

      {onTeam && clock.data && (
        <Panel
          className={cn(
            "flex items-center gap-3 p-4",
            open && "border-accent/50 bg-accent/[0.06]"
          )}
        >
          <Clock
            className={cn(
              "size-5 shrink-0",
              open ? "text-accent" : "text-fg-3"
            )}
          />
          <div className="min-w-0 flex-1">
            <div className="text-[14px] font-semibold text-fg">
              {open
                ? `On the clock${open.work_order_title ? `: ${open.work_order_title}` : ""}`
                : "Off the clock"}
            </div>
            <div className="text-[12px] text-fg-3">
              {hm(clock.data.today_minutes)} today ·{" "}
              {hm(clock.data.week_minutes)} this week
              {clock.data.missed_punches > 0 && (
                <>
                  {" "}
                  ·{" "}
                  <Link href="/console/my-time" className="text-warn">
                    {clock.data.missed_punches} missed punch
                    {clock.data.missed_punches === 1 ? "" : "es"}
                  </Link>
                </>
              )}
            </div>
          </div>
          {open && (
            <Button
              size="sm"
              variant="secondary"
              disabled={busy}
              onClick={stop}
            >
              <Square />
              Stop
            </Button>
          )}
        </Panel>
      )}

      {loading && <Skeleton className="h-64" />}
      {!loading && jobs.length === 0 && (
        <EmptyState
          icon={<CalendarCheck />}
          title="A clear day"
          description="Tasks given to you and visits booked with you show up here."
          className="py-12"
        />
      )}

      <ol className="space-y-3">
        {jobs.map((j, i) => {
          const here = open?.maintenance_ticket_id === j.ticket_id;
          const address = j.visit?.property_name ?? j.property_name;
          const phone = phoneLinks(j.visit?.with_phone);
          return (
            <li key={j.ticket_id}>
              <Panel
                className={cn("overflow-hidden", here && "border-accent/50")}
              >
                <div className="flex items-start gap-3 p-4">
                  <div className="figure flex size-8 shrink-0 items-center justify-center rounded-full bg-fill text-[13px] font-semibold text-fg-2">
                    {i + 1}
                  </div>
                  <div className="min-w-0 flex-1">
                    {j.visit && (
                      <div className="flex flex-wrap items-center gap-1.5 text-[12px]">
                        <Badge
                          tone={
                            j.visit.status === "confirmed" ? "good" : "neutral"
                          }
                        >
                          {j.visit.when_words
                            ? j.visit.when_words.replace(
                                /^[A-Za-z]{3}, [A-Za-z]{3} \d+, /,
                                ""
                              )
                            : "Time not picked yet"}
                        </Badge>
                        {j.visit.with_name && (
                          <span className="text-fg-3">
                            with {j.visit.with_name}
                          </span>
                        )}
                      </div>
                    )}
                    <Link
                      href={`/console/maintenance/${j.ticket_id}`}
                      className="mt-1 flex items-center gap-1 text-[15px] font-semibold text-fg"
                    >
                      <span className="truncate">{j.title}</span>
                      <ChevronRight className="size-4 shrink-0 text-fg-4" />
                    </Link>
                    <div className="mt-0.5 flex items-center gap-1 text-[13px] text-fg-3">
                      <MapPin className="size-3.5 shrink-0" />
                      <span className="truncate">
                        {j.property_name}
                        {j.location ? ` · ${j.location}` : ""}
                      </span>
                    </div>
                    {j.visit?.access_notes && (
                      <p className="mt-1 text-[12px] text-fg-2">
                        Getting in: {j.visit.access_notes}
                      </p>
                    )}
                    {j.minutes_today > 0 && (
                      <p className="mt-1 text-[12px] text-fg-3">
                        {hm(j.minutes_today)} logged here today
                      </p>
                    )}
                  </div>
                  {j.priority === "urgent" || j.priority === "high" ? (
                    <Badge tone="warn">{j.priority}</Badge>
                  ) : null}
                </div>

                {j.tasks.length > 0 && (
                  <ul className="divide-y divide-line border-t border-line">
                    {j.tasks.map((t) => (
                      <li
                        key={t.task_id}
                        className="flex items-center gap-3 px-4 py-2"
                      >
                        <button
                          type="button"
                          disabled={busy}
                          onClick={() => finishTask(t)}
                          aria-label={`Mark ${t.title} done`}
                          className={cn(
                            "flex size-6 shrink-0 items-center justify-center rounded-md border transition hover:border-accent",
                            t.status === "doing"
                              ? "border-accent"
                              : "border-line-strong"
                          )}
                        >
                          <Check className="size-4 text-transparent" />
                        </button>
                        <span className="min-w-0 flex-1 truncate text-[14px] text-fg">
                          {t.title}
                        </span>
                        {t.est_minutes ? (
                          <span className="text-[12px] text-fg-3">
                            {minutesLabel(t.est_minutes)}
                          </span>
                        ) : null}
                      </li>
                    ))}
                  </ul>
                )}

                <div className="flex flex-wrap items-center gap-2 border-t border-line bg-fill/40 px-4 py-2.5">
                  {onTeam &&
                    (here ? (
                      <Button
                        size="sm"
                        variant="secondary"
                        disabled={busy}
                        onClick={stop}
                      >
                        <Square />
                        Stop the clock
                      </Button>
                    ) : (
                      <Button
                        size="sm"
                        disabled={busy}
                        onClick={() => start(j)}
                      >
                        <Play />
                        {open ? "Switch here" : "Start the clock"}
                      </Button>
                    ))}
                  <a
                    href={mapLink(address)}
                    target="_blank"
                    rel="noreferrer"
                    className="inline-flex h-8 items-center gap-1 rounded-lg px-2 text-[12px] text-fg-2 hover:bg-fill"
                  >
                    <Navigation className="size-3.5" />
                    Directions
                  </a>
                  {phone && (
                    <a
                      href={phone.tel}
                      className="inline-flex h-8 items-center gap-1 rounded-lg px-2 text-[12px] text-fg-2 hover:bg-fill"
                    >
                      <Phone className="size-3.5" />
                      Call
                    </a>
                  )}
                  {j.visit?.status === "confirmed" && (
                    <span className="ml-auto flex gap-1">
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={busy}
                        onClick={() => visitOutcome(j.visit!, "no_show")}
                      >
                        Nobody home
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={busy}
                        onClick={() => visitOutcome(j.visit!, "done")}
                      >
                        Visit done
                      </Button>
                    </span>
                  )}
                  {j.tasks.length > 0 && !j.visit && (
                    <span className="ml-auto text-[12px] text-fg-3">
                      {j.tasks.length} task{j.tasks.length === 1 ? "" : "s"}{" "}
                      left
                    </span>
                  )}
                </div>
              </Panel>
            </li>
          );
        })}
      </ol>
    </div>
  );
}
