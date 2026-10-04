"use client";

// Code-required items for this property (alarms, extinguishers, water
// heater straps, lead paint, and so on), each as a routine on the schedule or
// not yet. "Add all" puts every item that applies on the schedule; the
// conditional ones (a pool, a boiler) are added one at a time.

import { useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CalendarPlus, ShieldCheck } from "lucide-react";
import { toast } from "sonner";
import { attention, cadenceWords, type MandateStatus } from "@/lib/attention";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";
import { why } from "./bits";

export function Mandates({
  propertyId,
  manage,
}: {
  propertyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const q = useQuery({
    queryKey: ["mandates", propertyId],
    queryFn: () => attention.mandates(propertyId),
  });
  const [busy, setBusy] = useState<string | null>(null);
  const [showBasis, setShowBasis] = useState<string | null>(null);

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ["mandates", propertyId] });
    void qc.invalidateQueries({ queryKey: ["plans"] });
    void qc.invalidateQueries({ queryKey: ["to-schedule"] });
  };

  async function add(keys: string[], label: string) {
    setBusy(label);
    try {
      const r = await attention.applyMandates(propertyId, keys);
      toast.success(
        r.created === 0
          ? "Already on the schedule"
          : `${r.created} routine${r.created === 1 ? "" : "s"} added`
      );
      refresh();
    } catch (e) {
      toast.error(why(e, "Couldn't add it"));
    } finally {
      setBusy(null);
    }
  }

  const items = q.data ?? [];
  const missing = items.filter((m) => !m.plan_id && !m.conditional);
  const have = items.filter((m) => m.plan_id).length;

  return (
    <Panel>
      <PanelHeader
        title="Required by code"
        description={
          items.length
            ? `${have} of ${items.length} on the schedule. Local code is the final word; these are the common ones.`
            : "Checks landlords owe by law or common code."
        }
        action={
          manage &&
          missing.length > 0 && (
            <Button
              size="sm"
              disabled={busy !== null}
              onClick={() => add([], "all")}
            >
              <CalendarPlus />
              Add all {missing.length}
            </Button>
          )
        }
      />
      {q.isLoading && (
        <div className="p-5">
          <Skeleton className="h-28" />
        </div>
      )}
      {q.isSuccess && items.length === 0 && (
        <EmptyState
          icon={<ShieldCheck />}
          title="Nothing applies"
          description="Set the property's state, units and year built to see what's required."
          className="py-6"
        />
      )}
      {items.length > 0 && (
        <ul className="divide-y divide-line">
          {items.map((m) => (
            <Row
              key={m.key}
              m={m}
              manage={manage}
              busy={busy === m.key || busy === "all"}
              open={showBasis === m.key}
              onToggle={() => setShowBasis(showBasis === m.key ? null : m.key)}
              onAdd={() => add([m.key], m.key)}
            />
          ))}
        </ul>
      )}
    </Panel>
  );
}

function Row({
  m,
  manage,
  busy,
  open,
  onToggle,
  onAdd,
}: {
  m: MandateStatus;
  manage: boolean;
  busy: boolean;
  open: boolean;
  onToggle: () => void;
  onAdd: () => void;
}) {
  return (
    <li className="px-5 py-3">
      <div className="flex items-start gap-3">
        <span
          className={cn(
            "mt-1.5 size-2 shrink-0 rounded-full",
            m.plan_id
              ? m.active
                ? "bg-good"
                : "bg-fg-4"
              : m.conditional
                ? "bg-fg-4"
                : m.priority === "high"
                  ? "bg-bad"
                  : "bg-warn"
          )}
          aria-hidden
        />
        <div className="min-w-0 flex-1">
          <button
            type="button"
            onClick={onToggle}
            className="text-left text-[14px] font-medium text-fg hover:underline"
          >
            {m.title}
          </button>
          <div className="text-xs text-fg-3">
            {cadenceWords(m.cadence_days)}
            {m.conditional ? " · only if the property has it" : ""}
            {m.plan_id && m.next_due_date ? ` · next ${m.next_due_date}` : ""}
          </div>
          {open && (
            <div className="mt-2 space-y-1 text-[13px] text-fg-2">
              <p>{m.description}</p>
              <p className="text-fg-3">{m.basis}</p>
            </div>
          )}
        </div>
        {m.plan_id ? (
          <Link
            href="/console/maintenance/schedule"
            className="shrink-0"
            aria-label={`${m.title} on the schedule`}
          >
            <Badge tone={m.active ? "good" : "neutral"}>
              {m.active ? "on the schedule" : "paused"}
            </Badge>
          </Link>
        ) : manage ? (
          <Button
            size="sm"
            variant="secondary"
            disabled={busy}
            onClick={onAdd}
            aria-label={`Add ${m.title}`}
          >
            <CalendarPlus />
            Add
          </Button>
        ) : (
          <Badge tone="warn">not scheduled</Badge>
        )}
      </div>
    </li>
  );
}
