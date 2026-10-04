"use client";

// The routine maintenance schedule: recurring work (filters, HVAC service,
// gutters, smoke detectors) that opens its own work order when it comes due,
// started from a job kit when one fits.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowLeft, CalendarClock, Pause, Play, Plus } from "lucide-react";
import { toast } from "sonner";
import { useAuth } from "@/lib/auth";
import { useProperties } from "@/lib/queries";
import { desk } from "@/lib/servicedesk";
import type { MaintenancePlan } from "@/lib/types";
import { Mandates } from "@/components/property/Mandates";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const CADENCES: [number, string][] = [
  [30, "Monthly"],
  [90, "Quarterly"],
  [182, "Twice a year"],
  [365, "Yearly"],
];

const field =
  "w-full rounded-xl border border-line bg-surface px-3 py-2 text-[13px] text-fg outline-none focus:border-accent";

function cadenceLabel(days: number): string {
  return CADENCES.find(([d]) => d === days)?.[1] ?? `Every ${days} days`;
}

function daysUntil(date: string): number {
  const d = new Date(`${date}T00:00:00`);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return Math.round((d.getTime() - today.getTime()) / 86_400_000);
}

export default function SchedulePage() {
  const { can } = useAuth();
  const manage = can("maintenance:manage");
  const qc = useQueryClient();
  const plans = useQuery({ queryKey: ["plans"], queryFn: desk.plans });
  const kits = useQuery({ queryKey: ["kits"], queryFn: desk.kits });
  const properties = useProperties();
  const [adding, setAdding] = useState(false);
  const [codeFor, setCodeFor] = useState("");

  const names = useMemo(
    () => new Map((properties.data ?? []).map((p) => [p.id, p.name])),
    [properties.data]
  );
  const kitNames = useMemo(
    () => new Map((kits.data ?? []).map((k) => [k.id, k.name])),
    [kits.data]
  );
  const groups = useMemo(() => {
    const rows = plans.data ?? [];
    const g: Record<string, MaintenancePlan[]> = {
      Overdue: [],
      "Next 30 days": [],
      Later: [],
      Paused: [],
    };
    for (const p of rows) {
      if (!p.active) g.Paused.push(p);
      else {
        const d = daysUntil(p.next_due_date);
        (d < 0 ? g.Overdue : d <= 30 ? g["Next 30 days"] : g.Later).push(p);
      }
    }
    return Object.entries(g).filter(([, v]) => v.length > 0);
  }, [plans.data]);

  async function toggle(p: MaintenancePlan) {
    try {
      await desk.updatePlan(p.id, { active: !p.active });
      void qc.invalidateQueries({ queryKey: ["plans"] });
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't change it");
    }
  }

  return (
    <div className="space-y-6">
      <Link
        href="/console/maintenance"
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 transition hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        Service desk
      </Link>
      <PageHeader
        eyebrow="Service desk"
        title="Maintenance schedule"
        description="Routine work opens its own work order on the due date, with the kit's tasks and parts."
        actions={
          manage &&
          !adding && (
            <Button onClick={() => setAdding(true)}>
              <Plus />
              Add routine
            </Button>
          )
        }
      />

      {adding && (
        <NewRoutine
          onDone={() => {
            setAdding(false);
            void qc.invalidateQueries({ queryKey: ["plans"] });
          }}
          onCancel={() => setAdding(false)}
        />
      )}

      {plans.isLoading && <Skeleton className="h-48 rounded-2xl" />}
      {plans.data?.length === 0 && !adding && (
        <Panel>
          <EmptyState
            icon={<CalendarClock />}
            title="Nothing scheduled"
            description="Add filter changes, HVAC service, gutter cleaning, smoke detector checks."
          />
        </Panel>
      )}

      <div className="flex flex-col gap-2 rounded-2xl border border-dashed border-line px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <div className="text-[14px] font-semibold text-fg">
            Required by code
          </div>
          <div className="text-xs text-fg-3">
            Smoke and CO alarms, extinguishers, water heater straps, lead paint
            and the rest, as routines. Pick a property.
          </div>
        </div>
        <select
          aria-label="Property for code-required items"
          className={cn(field, "w-auto py-1.5")}
          value={codeFor}
          onChange={(e) => setCodeFor(e.target.value)}
        >
          <option value="">Choose a property…</option>
          {(properties.data ?? []).map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </div>
      {codeFor && <Mandates propertyId={codeFor} manage={manage} />}

      {groups.map(([label, rows]) => (
        <Panel key={label}>
          <PanelHeader
            title={label}
            description={`${rows.length} routine${rows.length === 1 ? "" : "s"}`}
          />
          <ul className="divide-y divide-line p-2 pt-3">
            {rows.map((p) => {
              const d = daysUntil(p.next_due_date);
              return (
                <li key={p.id} className="flex items-center gap-3 px-3 py-2.5">
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[14px] font-medium text-fg">
                      {p.title}
                    </div>
                    <div className="truncate text-xs text-fg-3">
                      {names.get(p.property_id) ?? "Property"} ·{" "}
                      {cadenceLabel(p.cadence_days)}
                      {p.mandate_key ? " · required by code" : ""}
                      {p.issue_template_id && kitNames.get(p.issue_template_id)
                        ? ` · kit: ${kitNames.get(p.issue_template_id)}`
                        : ""}
                    </div>
                  </div>
                  {p.last_ticket_id && (
                    <Link
                      href={`/console/maintenance/${p.last_ticket_id}`}
                      className="hidden text-xs text-fg-3 hover:text-fg sm:block"
                    >
                      Last work order
                    </Link>
                  )}
                  <Badge
                    tone={
                      !p.active
                        ? "neutral"
                        : d < 0
                          ? "bad"
                          : d <= 7
                            ? "warn"
                            : "neutral"
                    }
                  >
                    {!p.active
                      ? "paused"
                      : d < 0
                        ? `${-d}d overdue`
                        : d === 0
                          ? "due today"
                          : `in ${d}d`}
                  </Badge>
                  <span className="figure hidden w-24 text-right text-xs text-fg-3 md:block">
                    {p.next_due_date}
                  </span>
                  {manage && (
                    <button
                      type="button"
                      onClick={() => toggle(p)}
                      aria-label={
                        p.active ? `Pause ${p.title}` : `Resume ${p.title}`
                      }
                      className="rounded-lg p-1.5 text-fg-3 transition hover:bg-fill-2 hover:text-fg"
                    >
                      {p.active ? (
                        <Pause className="size-4" />
                      ) : (
                        <Play className="size-4" />
                      )}
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        </Panel>
      ))}
    </div>
  );
}

function NewRoutine({
  onDone,
  onCancel,
}: {
  onDone: () => void;
  onCancel: () => void;
}) {
  const properties = useProperties();
  const kits = useQuery({ queryKey: ["kits"], queryFn: desk.kits });
  const [propertyId, setPropertyId] = useState("");
  const [title, setTitle] = useState("");
  const [cadence, setCadence] = useState(90);
  const [due, setDue] = useState(new Date().toISOString().slice(0, 10));
  const [kitId, setKitId] = useState("");
  const [busy, setBusy] = useState(false);

  async function save() {
    if (!propertyId || !title.trim()) {
      toast.error("Choose a property and name the routine.");
      return;
    }
    setBusy(true);
    try {
      await desk.createPlan({
        property_id: propertyId,
        title: title.trim(),
        cadence_days: cadence,
        next_due_date: due,
        issue_template_id: kitId || undefined,
        category: kits.data?.find((k) => k.id === kitId)?.category,
      });
      toast.success("Routine added");
      onDone();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't add it");
      setBusy(false);
    }
  }

  return (
    <Panel className="p-5">
      <div className="grid gap-3 md:grid-cols-2">
        <select
          className={field}
          value={propertyId}
          onChange={(e) => setPropertyId(e.target.value)}
          aria-label="Property"
        >
          <option value="">Property…</option>
          {(properties.data ?? []).map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <input
          className={field}
          placeholder="e.g. Replace HVAC filters"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
        />
        <div className="flex gap-1 rounded-xl bg-fill p-1">
          {CADENCES.map(([d, l]) => (
            <button
              key={d}
              type="button"
              onClick={() => setCadence(d)}
              className={cn(
                "flex-1 rounded-lg px-2 py-1.5 text-xs font-medium transition",
                cadence === d
                  ? "bg-surface text-fg shadow-sm"
                  : "text-fg-3 hover:text-fg"
              )}
            >
              {l}
            </button>
          ))}
        </div>
        <label className="flex items-center gap-2 text-[13px] text-fg-2">
          First due
          <input
            type="date"
            className={field}
            value={due}
            onChange={(e) => setDue(e.target.value)}
          />
        </label>
        <select
          className={cn(field, "md:col-span-2")}
          value={kitId}
          onChange={(e) => {
            setKitId(e.target.value);
            const k = kits.data?.find((x) => x.id === e.target.value);
            if (k && !title.trim()) setTitle(k.name);
          }}
          aria-label="Job kit"
        >
          <option value="">No kit: a work order with just the title</option>
          {(kits.data ?? []).map((k) => (
            <option key={k.id} value={k.id}>
              {k.name}
              {k.tasks.length ? ` · ${k.tasks.length} tasks` : ""}
            </option>
          ))}
        </select>
      </div>
      <div className="mt-4 flex justify-end gap-2">
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button onClick={save} disabled={busy}>
          Add routine
        </Button>
      </div>
    </Panel>
  );
}
