"use client";

// Turnovers: every unit being turned from move-out to move-in ready, with its
// progress, days vacant and cost. A finished move-out inspection starts one on
// its own; the office can also start one by hand.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { motion } from "motion/react";
import { ListChecks, PaintRoller, Play, Search } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import { useProperties } from "@/lib/queries";
import { turns, type Turn } from "@/lib/turns";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat, Tabs } from "@/components/ui/data-table";
import { fieldClass, Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

type Status = "active" | "done" | "cancelled";

const TABS = [
  ["active", "In progress"],
  ["done", "Done"],
  ["cancelled", "Cancelled"],
] as const;

export default function TurnsPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const manage = can("maintenance:manage");
  const [status, setStatus] = useState<Status>("active");
  const [q, setQ] = useState("");
  const list = useQuery({
    queryKey: ["turns", status],
    queryFn: () => turns.list({ status }),
    enabled: scoped && can("maintenance:read"),
  });
  const rows = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return (list.data ?? []).filter(
      (t) =>
        !needle ||
        t.property_name.toLowerCase().includes(needle) ||
        (t.unit_number ?? "").toLowerCase().includes(needle)
    );
  }, [list.data, q]);
  const active = (list.data ?? []).filter((t) => t.status === "active");
  const late = active.filter((t) => t.overdue).length;
  const avgDays = active.length
    ? Math.round(active.reduce((n, t) => n + t.days_open, 0) / active.length)
    : 0;
  const cost = active.reduce((n, t) => n + t.cost_cents, 0);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Maintenance"
        title="Turnovers"
        description="Every step from move-out to move-in ready, in order."
        actions={
          manage && (
            <Button variant="secondary" asChild>
              <Link href="/console/turns/templates">
                <ListChecks />
                Edit steps
              </Link>
            </Button>
          )
        }
      />

      {status === "active" && (
        <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
          <Stat label="In progress" value={list.data ? active.length : "—"} />
          <Stat
            label="Past target"
            value={list.data ? late : "—"}
            tone={late ? "warn" : undefined}
          />
          <Stat label="Average days vacant" value={list.data ? avgDays : "—"} />
          <Stat label="Spent so far" value={list.data ? usd(cost) : "—"} />
        </section>
      )}

      {manage && <StartTurn enabled={scoped} />}

      <Panel className="overflow-hidden">
        <div className="flex flex-col gap-3 border-b border-line p-3 sm:flex-row sm:items-center sm:justify-between">
          <Tabs tabs={TABS} value={status} onChange={setStatus} />
          <div className="relative sm:w-64">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Search property or unit"
              aria-label="Search turnovers"
              className="pl-9"
            />
          </div>
        </div>
        {list.isLoading && (
          <div className="space-y-2 p-3">
            {Array.from({ length: 4 }, (_, i) => (
              <Skeleton key={i} className="h-14" />
            ))}
          </div>
        )}
        {list.error && (
          <p className="p-4 text-[13px] text-bad">
            Couldn&apos;t load turnovers: {list.error.message}
          </p>
        )}
        {list.data && rows.length === 0 && (
          <EmptyState
            icon={<PaintRoller />}
            title={
              q
                ? "Nothing matches"
                : status === "active"
                  ? "No turns in progress"
                  : `No ${status} turnovers`
            }
            description={
              status === "active" && !q
                ? "A finished move-out inspection starts one on its own."
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
              <TurnRow t={t} />
            </motion.li>
          ))}
        </ul>
      </Panel>
    </div>
  );
}

function TurnRow({ t }: { t: Turn }) {
  const pct = t.total ? Math.round((t.done / t.total) * 100) : 0;
  return (
    <Link
      href={`/console/turns/${t.id}`}
      className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 transition hover:bg-fill-2"
    >
      <div className="min-w-0 flex-1">
        <div className="truncate text-[14px] font-medium text-fg">
          {t.property_name} · Unit {t.unit_number ?? "—"}
        </div>
        <div className="truncate text-xs text-fg-3">
          Started {t.started_on}
          {t.target_date ? ` · target ${t.target_date}` : ""}
          {t.finished_on ? ` · finished ${t.finished_on}` : ""}
        </div>
      </div>
      <div className="w-36">
        <div className="h-1.5 overflow-hidden rounded-full bg-fill">
          <div
            className="h-full rounded-full bg-accent"
            style={{ width: `${pct}%` }}
          />
        </div>
        <div className="mt-1 text-[11px] text-fg-3">
          {t.done} of {t.total} steps
        </div>
      </div>
      <span className="figure hidden w-16 text-right text-[13px] text-fg-2 sm:block">
        {t.days_open} {t.days_open === 1 ? "day" : "days"}
      </span>
      <span className="figure hidden w-20 text-right text-[13px] text-fg-2 md:block">
        {t.cost_label}
      </span>
      {t.overdue ? (
        <Badge tone="bad">past target</Badge>
      ) : (
        <Badge
          tone={
            t.status === "done"
              ? "good"
              : t.status === "cancelled"
                ? "neutral"
                : "info"
          }
        >
          {t.status === "active" ? "in progress" : t.status}
        </Badge>
      )}
    </Link>
  );
}

/** Start a turn by hand: pick the property, then the unit. */
function StartTurn({ enabled }: { enabled: boolean }) {
  const qc = useQueryClient();
  const properties = useProperties({ enabled });
  const [propertyId, setPropertyId] = useState("");
  const [unitId, setUnitId] = useState("");
  const [busy, setBusy] = useState(false);
  const units = useQuery({
    queryKey: ["properties", propertyId, "units"],
    queryFn: () => api.units(propertyId),
    enabled: !!propertyId,
  });

  async function go() {
    setBusy(true);
    try {
      await turns.start(unitId, {});
      toast.success("Turnover started");
      setUnitId("");
      void qc.invalidateQueries({ queryKey: ["turns"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't start it");
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel className="flex flex-wrap items-center gap-3 p-3">
      <span className="px-1 text-[13px] font-medium text-fg">Start a turn</span>
      <select
        aria-label="Property"
        className={fieldClass}
        value={propertyId}
        onChange={(e) => {
          setPropertyId(e.target.value);
          setUnitId("");
        }}
      >
        <option value="">Property</option>
        {properties.data?.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
      <select
        aria-label="Unit"
        className={fieldClass}
        value={unitId}
        disabled={!propertyId}
        onChange={(e) => setUnitId(e.target.value)}
      >
        <option value="">Unit</option>
        {units.data?.map((u) => (
          <option key={u.id} value={u.id}>
            {u.unit_number}
          </option>
        ))}
      </select>
      <Button size="sm" onClick={go} disabled={!unitId} loading={busy}>
        <Play />
        Start
      </Button>
    </Panel>
  );
}
