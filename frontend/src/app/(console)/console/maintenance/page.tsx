"use client";

// The service desk: every open work order the viewer can see (property
// managers see their own properties'), what's urgent or past its SLA, and the
// way into a new work order from a job kit.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import {
  AlarmClock,
  CalendarClock,
  ClipboardList,
  Hourglass,
  Plus,
  Search,
  Siren,
  Wrench,
} from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useProperties } from "@/lib/queries";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import type { MaintenanceTicket } from "@/lib/types";
import { cn } from "@/lib/utils";

// Matches the server's open statuses (routes/maintenance OPEN_STATUSES).
const OPEN = ["open", "triage", "scheduled", "in_progress", "on_hold"];

type View = "open" | "urgent" | "waiting" | "done";

const VIEWS: { key: View; label: string }[] = [
  { key: "open", label: "Open" },
  { key: "urgent", label: "Urgent" },
  { key: "waiting", label: "Waiting" },
  { key: "done", label: "Done" },
];

function inView(t: MaintenanceTicket, v: View): boolean {
  const open = OPEN.includes(t.status);
  switch (v) {
    case "open":
      return open;
    case "urgent":
      return open && (t.priority === "urgent" || t.priority === "high");
    case "waiting":
      return t.status === "on_hold" || !!t.waiting_on;
    case "done":
      return !open;
  }
}

function ago(iso: string): string {
  const d = (Date.now() - new Date(iso).getTime()) / 86_400_000;
  if (d < 1) return "today";
  if (d < 2) return "yesterday";
  return `${Math.floor(d)}d ago`;
}

export default function ServiceDeskPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const tickets = useQuery({
    queryKey: ["tickets"],
    queryFn: () => api.tickets(),
    enabled: scoped,
  });
  const properties = useProperties({ enabled: scoped });
  const [view, setView] = useState<View>("open");
  const [q, setQ] = useState("");

  const names = useMemo(
    () => new Map((properties.data ?? []).map((p) => [p.id, p.name])),
    [properties.data]
  );
  const all = useMemo(() => tickets.data ?? [], [tickets.data]);
  const counts = useMemo(
    () => ({
      open: all.filter((t) => inView(t, "open")).length,
      urgent: all.filter((t) => inView(t, "urgent")).length,
      waiting: all.filter((t) => inView(t, "waiting")).length,
      late: all.filter(
        (t) =>
          OPEN.includes(t.status) &&
          (t.sla_resolve_state === "breached" ||
            t.sla_response_state === "breached")
      ).length,
    }),
    [all]
  );
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return all
      .filter((t) => inView(t, view))
      .filter(
        (t) =>
          !needle ||
          t.title.toLowerCase().includes(needle) ||
          (names.get(t.property_id) ?? "").toLowerCase().includes(needle)
      );
  }, [all, view, q, names]);

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
          icon={<Hourglass />}
          label="Waiting on something"
          value={counts.waiting}
        />
        <Stat
          icon={<AlarmClock />}
          label="Past SLA"
          value={counts.late}
          tone={counts.late ? "warn" : undefined}
        />
      </section>

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
              </button>
            ))}
          </div>
          <div className="relative sm:w-72">
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
