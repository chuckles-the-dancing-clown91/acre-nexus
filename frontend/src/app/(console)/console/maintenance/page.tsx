"use client";

// The service desk: every open work order the viewer can see (property
// managers see their own properties'), what's urgent or past its SLA, and the
// way into a new work order from a job kit.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  AlarmClock,
  CalendarClock,
  ClipboardList,
  Check,
  HardHat,
  Play,
  Plus,
  Search,
  Siren,
  User,
  Wrench,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import {
  desk,
  inQueueView,
  loadLabel,
  minutesLabel,
  tradeLabel,
  type QueueView,
  type Tech,
} from "@/lib/servicedesk";
import { useProperties } from "@/lib/queries";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { ToSchedule } from "@/components/desk/ToSchedule";
import { attention } from "@/lib/attention";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

// Matches the server's open statuses (routes/maintenance OPEN_STATUSES).
const OPEN = ["open", "triage", "scheduled", "in_progress", "on_hold"];

const VIEWS: { key: QueueView; label: string }[] = [
  { key: "mine", label: "Mine" },
  { key: "open", label: "Open" },
  { key: "unassigned", label: "Unassigned" },
  { key: "urgent", label: "Urgent" },
  { key: "waiting", label: "Waiting" },
  { key: "done", label: "Done" },
];

function ago(iso: string): string {
  const d = (Date.now() - new Date(iso).getTime()) / 86_400_000;
  if (d < 1) return "today";
  if (d < 2) return "yesterday";
  return `${Math.floor(d)}d ago`;
}

export default function ServiceDeskPage() {
  const { can, user } = useAuth();
  const me = user?.id ?? null;
  const manage = can("maintenance:manage");
  const scoped = useHasTenantScope();
  const qc = useQueryClient();
  const tickets = useQuery({
    queryKey: ["tickets"],
    queryFn: () => api.tickets(),
    enabled: scoped,
  });
  const properties = useProperties({ enabled: scoped });
  const techs = useQuery({
    queryKey: ["techs", "all"],
    queryFn: () => desk.techs(),
    enabled: scoped,
  });
  const myTasks = useQuery({
    queryKey: ["my-queue"],
    queryFn: desk.queue,
    enabled: scoped,
  });
  const toSchedule = useQuery({
    queryKey: ["to-schedule"],
    queryFn: attention.toSchedule,
    enabled: scoped,
  });
  const needDates =
    (toSchedule.data?.plans.length ?? 0) +
    (toSchedule.data?.tickets.length ?? 0);
  const [view, setView] = useState<QueueView>("open");
  const [who, setWho] = useState<string>("");
  const [q, setQ] = useState("");

  const names = useMemo(
    () => new Map((properties.data ?? []).map((p) => [p.id, p.name])),
    [properties.data]
  );
  const all = useMemo(() => tickets.data ?? [], [tickets.data]);
  const counts = useMemo(() => {
    const n = (v: QueueView) => all.filter((t) => inQueueView(t, v, me)).length;
    return {
      mine: n("mine") + (myTasks.data?.tasks.length ?? 0),
      open: n("open"),
      unassigned: n("unassigned"),
      urgent: n("urgent"),
      waiting: n("waiting"),
      done: 0,
      late: all.filter(
        (t) =>
          OPEN.includes(t.status) &&
          (t.sla_resolve_state === "breached" ||
            t.sla_response_state === "breached")
      ).length,
    };
  }, [all, me, myTasks.data]);
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return all
      .filter((t) => inQueueView(t, view, me))
      .filter((t) =>
        who === ""
          ? true
          : who === "vendors"
            ? !!t.assignee_entity_id && !t.assignee_user_id
            : t.assignee_user_id === who
      )
      .filter(
        (t) =>
          !needle ||
          t.title.toLowerCase().includes(needle) ||
          (names.get(t.property_id) ?? "").toLowerCase().includes(needle)
      );
  }, [all, view, who, q, names, me]);

  async function assign(ticketId: string, userId: string) {
    try {
      await api.updateTicket(
        ticketId,
        userId ? { assignee_user_id: userId } : { clear_assignee_user: true }
      );
      void qc.invalidateQueries({ queryKey: ["tickets"] });
      void qc.invalidateQueries({ queryKey: ["techs"] });
      void qc.invalidateQueries({ queryKey: ["my-queue"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't assign it");
    }
  }

  async function step(
    t: { ticket_id: string; task_id: string },
    status: "doing" | "done"
  ) {
    try {
      await desk.updateTask(t.ticket_id, t.task_id, { status });
      void qc.invalidateQueries({ queryKey: ["my-queue"] });
      void qc.invalidateQueries({ queryKey: ["tickets"] });
      void qc.invalidateQueries({ queryKey: ["techs"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "That didn't work");
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Maintenance"
        title="Service desk"
        description="Work orders, the people and vendors on them, and what they're costing."
        actions={
          <>
            <Button variant="secondary" asChild>
              <Link href="/console/maintenance/schedule">
                <CalendarClock />
                Schedule
              </Link>
            </Button>
            {can("maintenance:manage") && (
              <Button asChild>
                <Link href="/console/maintenance/new">
                  <Plus />
                  New work order
                </Link>
              </Button>
            )}
          </>
        }
      />

      <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <Stat icon={<Wrench />} label="Open" value={counts.open} />
        <Stat
          icon={<Siren />}
          label="Urgent or high"
          value={counts.urgent}
          tone={counts.urgent ? "bad" : undefined}
        />
        <Stat
          icon={<CalendarClock />}
          label="To schedule"
          value={needDates}
          tone={needDates ? "warn" : undefined}
        />
        <Stat
          icon={<AlarmClock />}
          label="Past SLA"
          value={counts.late}
          tone={counts.late ? "warn" : undefined}
        />
      </section>

      {view === "mine" && (myTasks.data?.tasks.length ?? 0) > 0 && (
        <Panel className="overflow-hidden">
          <div className="border-b border-line px-4 py-3">
            <div className="text-[14px] font-semibold text-fg">My tasks</div>
            <div className="text-xs text-fg-3">
              Started first, then urgent, then soonest due.
            </div>
          </div>
          <ul className="divide-y divide-line">
            {myTasks.data?.tasks.map((t) => (
              <li key={t.task_id} className="flex items-center gap-3 px-4 py-3">
                <span
                  className={cn(
                    "size-2 shrink-0 rounded-full",
                    t.priority === "urgent"
                      ? "bg-bad"
                      : t.priority === "high"
                        ? "bg-warn"
                        : "bg-fg-4"
                  )}
                  aria-hidden
                />
                <Link
                  href={`/console/maintenance/${t.ticket_id}`}
                  className="min-w-0 flex-1"
                >
                  <div className="truncate text-[14px] font-medium text-fg">
                    {t.title}
                  </div>
                  <div className="truncate text-xs text-fg-3">
                    {t.property_name}
                    {t.location ? ` · ${t.location}` : ""} · {t.ticket_title}
                    {t.est_minutes ? ` · ${minutesLabel(t.est_minutes)}` : ""}
                    {t.due_date ? ` · due ${t.due_date}` : ""}
                  </div>
                </Link>
                <Badge className="hidden sm:inline-flex">
                  {tradeLabel(t.trade)}
                </Badge>
                {t.status === "todo" ? (
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() => step(t, "doing")}
                  >
                    <Play />
                    Start
                  </Button>
                ) : (
                  <Button size="sm" onClick={() => step(t, "done")}>
                    <Check />
                    Done
                  </Button>
                )}
              </li>
            ))}
          </ul>
        </Panel>
      )}

      <ToSchedule manage={manage} names={names} enabled={scoped} />

      {manage && (techs.data?.length ?? 0) > 0 && (
        <TeamStrip techs={techs.data ?? []} who={who} onPick={setWho} />
      )}

      <Panel className="overflow-hidden">
        <div className="flex flex-col gap-3 border-b border-line p-3 sm:flex-row sm:items-center sm:justify-between">
          <div className="flex gap-1 rounded-xl bg-fill p-1" role="tablist">
            {VIEWS.map((v) => (
              <button
                key={v.key}
                role="tab"
                aria-selected={view === v.key}
                onClick={() => setView(v.key)}
                className={cn(
                  "rounded-lg px-3 py-1.5 text-[13px] font-medium transition",
                  view === v.key
                    ? "bg-surface text-fg shadow-sm"
                    : "text-fg-3 hover:text-fg"
                )}
              >
                {v.label}
                {v.key !== "done" && counts[v.key] > 0 && (
                  <span className="figure ml-1.5 text-[11px] text-fg-3">
                    {counts[v.key]}
                  </span>
                )}
              </button>
            ))}
          </div>
          <select
            aria-label="Whose work"
            value={who}
            onChange={(e) => setWho(e.target.value)}
            className="rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg sm:ml-auto"
          >
            <option value="">Everyone</option>
            <option value="vendors">Vendors</option>
            {techs.data?.map((p) => (
              <option key={p.user_id} value={p.user_id}>
                {p.name}
                {p.open_tickets ? ` (${p.open_tickets})` : ""}
              </option>
            ))}
          </select>
          <div className="relative sm:w-64">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Search work orders or properties"
              aria-label="Search work orders"
              className="pl-9"
            />
          </div>
        </div>

        {tickets.isLoading && (
          <div className="space-y-2 p-3">
            {Array.from({ length: 5 }, (_, i) => (
              <Skeleton key={i} className="h-14" />
            ))}
          </div>
        )}
        {tickets.error && (
          <p className="p-4 text-[13px] text-bad">
            Couldn&apos;t load work orders: {tickets.error.message}
          </p>
        )}
        {tickets.data && rows.length === 0 && (
          <EmptyState
            icon={<ClipboardList />}
            title={view === "open" ? "Nothing open" : "Nothing here"}
            description={
              view === "open"
                ? "Every work order is closed. Start one from a job kit."
                : undefined
            }
          />
        )}
        <ul className="divide-y divide-line">
          {rows.map((t, i) => (
            <motion.li
              key={t.id}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: Math.min(i, 15) * 0.02, duration: 0.3 }}
            >
              <Link
                href={`/console/maintenance/${t.id}`}
                className="flex items-center gap-3 px-4 py-3 transition hover:bg-fill-2"
              >
                <span
                  className={cn(
                    "size-2 shrink-0 rounded-full",
                    t.priority === "urgent"
                      ? "bg-bad"
                      : t.priority === "high"
                        ? "bg-warn"
                        : "bg-fg-4"
                  )}
                  aria-hidden
                />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[14px] font-medium text-fg">
                    {t.title}
                  </div>
                  <div className="truncate text-xs text-fg-3">
                    {names.get(t.property_id) ?? "Property"}
                    {t.location ? ` · ${t.location}` : ""} · {t.category} ·{" "}
                    {ago(t.created_at)}
                  </div>
                </div>
                {t.waiting_on && (
                  <Badge tone="warn" className="hidden sm:inline-flex">
                    waiting on {t.waiting_on}
                  </Badge>
                )}
                {(t.tasks_total ?? 0) > 0 && (
                  <span
                    className="figure hidden text-xs text-fg-3 sm:block"
                    title="Tasks done"
                  >
                    {t.tasks_done}/{t.tasks_total}
                  </span>
                )}
                <span className="hidden w-36 shrink-0 md:block">
                  {manage && OPEN.includes(t.status) ? (
                    <select
                      aria-label={`Assign ${t.title}`}
                      value={t.assignee_user_id ?? ""}
                      onClick={(e) => {
                        // The row is a link; picking someone isn't opening it.
                        e.preventDefault();
                        e.stopPropagation();
                      }}
                      onChange={(e) => {
                        e.preventDefault();
                        void assign(t.id, e.target.value);
                      }}
                      className="w-full rounded-lg border border-line bg-surface px-2 py-1 text-xs text-fg-2"
                    >
                      <option value="">
                        {t.assignee_kind === "vendor"
                          ? `Vendor: ${t.assignee_name ?? ""}`
                          : "Unassigned"}
                      </option>
                      {techs.data?.map((p) => (
                        <option key={p.user_id} value={p.user_id}>
                          {p.name}
                        </option>
                      ))}
                    </select>
                  ) : (
                    <Who t={t} />
                  )}
                </span>
                {t.cost_label && (
                  <span className="figure hidden text-[13px] text-fg-2 md:block">
                    {t.cost_label}
                  </span>
                )}
                <Badge tone={statusTone(t.status)}>
                  {t.status.replace("_", " ")}
                </Badge>
              </Link>
            </motion.li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}

function Who({
  t,
}: {
  t: { assignee_name?: string | null; assignee_kind?: string | null };
}) {
  if (!t.assignee_name)
    return <span className="text-xs text-fg-4">Unassigned</span>;
  return (
    <span className="inline-flex max-w-full items-center gap-1 text-xs text-fg-2">
      {t.assignee_kind === "vendor" ? (
        <HardHat className="size-3.5 shrink-0" />
      ) : (
        <User className="size-3.5 shrink-0" />
      )}
      <span className="truncate">{t.assignee_name}</span>
    </span>
  );
}

/** The team and what each person has, one press to see their queue. */
function TeamStrip({
  techs,
  who,
  onPick,
}: {
  techs: Tech[];
  who: string;
  onPick: (id: string) => void;
}) {
  return (
    <Panel className="p-3">
      <div className="mb-2 px-1 text-xs font-medium text-fg-3">Team</div>
      <div className="flex flex-wrap gap-2">
        {techs.map((p) => {
          const on = who === p.user_id;
          return (
            <button
              key={p.user_id}
              type="button"
              onClick={() => onPick(on ? "" : p.user_id)}
              aria-pressed={on}
              className={cn(
                "flex items-center gap-2 rounded-xl border px-2.5 py-1.5 text-left transition",
                on
                  ? "border-accent bg-accent/10"
                  : "border-line hover:bg-fill-2"
              )}
            >
              <span className="flex size-7 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[11px] font-semibold text-accent">
                {p.name
                  .split(" ")
                  .map((w) => w[0])
                  .slice(0, 2)
                  .join("")}
              </span>
              <span className="min-w-0">
                <span className="block truncate text-[13px] font-medium text-fg">
                  {p.name}
                </span>
                <span className="block text-[11px] text-fg-3">
                  {loadLabel(p)}
                </span>
              </span>
            </button>
          );
        })}
      </div>
    </Panel>
  );
}

function Stat({
  icon,
  label,
  value,
  tone,
}: {
  icon: React.ReactNode;
  label: string;
  value: number;
  tone?: "bad" | "warn";
}) {
  return (
    <Panel className="flex items-center gap-4 p-4">
      <span
        className={cn(
          "flex size-10 items-center justify-center rounded-xl border [&_svg]:size-[18px]",
          tone === "bad"
            ? "border-bad/30 bg-bad/10 text-bad"
            : tone === "warn"
              ? "border-warn/30 bg-warn/10 text-warn"
              : "border-line bg-fill text-fg-2"
        )}
      >
        {icon}
      </span>
      <div>
        <div className="figure text-[24px] leading-none font-semibold text-fg">
          {value}
        </div>
        <div className="mt-1 text-xs text-fg-3">{label}</div>
      </div>
    </Panel>
  );
}
