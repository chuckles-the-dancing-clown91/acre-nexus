"use client";

// Getting this house ready: the onboarding checklist, ticked from the data,
// and the property record's suggested values to apply.

import { useState } from "react";
import Link from "next/link";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckCircle2, ChevronRight, Circle, Sparkles } from "lucide-react";
import { toast } from "sonner";
import { checklist, shown } from "@/lib/checklist";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

export function Readiness({
  propertyId,
  manage,
}: {
  propertyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const list = useQuery({
    queryKey: ["property", propertyId, "checklist"],
    queryFn: () => checklist.get(propertyId),
  });
  const auto = useQuery({
    queryKey: ["property", propertyId, "autofill"],
    queryFn: () => checklist.autofill(propertyId),
  });
  const [picked, setPicked] = useState<Set<string> | null>(null);
  const proposals = auto.data?.proposals ?? [];
  const chosen = picked ?? new Set(proposals.map((p) => p.field));
  const apply = useMutation({
    mutationFn: () => checklist.apply(propertyId, [...chosen]),
    onSuccess: (r) => {
      qc.setQueryData(["property", propertyId, "autofill"], r);
      setPicked(null);
      void qc.invalidateQueries({ queryKey: ["property", propertyId] });
      toast.success("Applied from the property record");
    },
    onError: (e) =>
      toast.error(e instanceof Error ? e.message : "Couldn't apply them"),
  });
  const c = list.data;
  if (!c) return null;
  const ready = c.required_done === c.required && proposals.length === 0;
  if (ready) {
    return (
      <Panel className="flex items-center gap-3 px-5 py-3.5">
        <CheckCircle2 className="size-5 text-good" />
        <div className="text-[13px] text-fg-2">
          <span className="font-medium text-fg">Ready.</span> Every onboarding
          step is done.
        </div>
      </Panel>
    );
  }
  const pct = Math.round((c.required_done / Math.max(c.required, 1)) * 100);
  return (
    <Panel>
      <PanelHeader
        title="Getting this house ready"
        description={`${c.required_done} of ${c.required} steps done`}
        action={<span className="font-mono text-[13px] text-fg-3">{pct}%</span>}
      />
      <div className="mx-5 mb-3 h-1.5 overflow-hidden rounded-full bg-fill-2">
        <div
          className="h-full rounded-full bg-accent transition-all"
          style={{ width: `${pct}%` }}
        />
      </div>
      {proposals.length > 0 && (
        <div className="mx-5 mb-4 rounded-xl border border-accent/30 bg-accent/5 p-3">
          <div className="flex items-center gap-2 text-[13px] font-medium text-fg">
            <Sparkles className="size-4 text-accent" />
            From the property record
          </div>
          <ul className="mt-2 space-y-1.5">
            {proposals.map((p) => (
              <li key={p.field}>
                <label className="flex items-start gap-2 text-[13px]">
                  <input
                    type="checkbox"
                    className="mt-0.5 size-4 accent-[var(--accent)]"
                    disabled={!manage}
                    checked={chosen.has(p.field)}
                    onChange={(e) => {
                      const next = new Set(chosen);
                      if (e.target.checked) next.add(p.field);
                      else next.delete(p.field);
                      setPicked(next);
                    }}
                  />
                  <span className="min-w-0">
                    <span className="font-medium text-fg">{p.label}:</span>{" "}
                    <span className="text-fg-3 line-through decoration-fg-4">
                      {shown(p, p.current)}
                    </span>{" "}
                    <span className="text-fg">{shown(p, p.proposed)}</span>{" "}
                    <span className="text-[11px] text-fg-4">({p.source})</span>
                  </span>
                </label>
              </li>
            ))}
          </ul>
          {manage && (
            <Button
              size="sm"
              className="mt-3"
              loading={apply.isPending}
              disabled={chosen.size === 0}
              onClick={() => apply.mutate()}
            >
              Apply {chosen.size === proposals.length ? "all" : chosen.size}
            </Button>
          )}
        </div>
      )}
      <ol className="divide-y divide-line">
        {c.steps.map((s) => (
          <li key={s.key}>
            <Link
              href={s.href}
              className={cn(
                "flex items-center gap-3 px-5 py-2.5 hover:bg-surface-2",
                c.next === s.key && "bg-accent/5"
              )}
            >
              {s.done ? (
                <CheckCircle2
                  className="size-4 shrink-0 text-good"
                  aria-label="Done"
                />
              ) : (
                <Circle
                  className="size-4 shrink-0 text-fg-4"
                  aria-label="To do"
                />
              )}
              <span className="min-w-0 flex-1">
                <span
                  className={cn(
                    "block text-[13px] font-medium",
                    s.done ? "text-fg-3" : "text-fg"
                  )}
                >
                  {s.title}
                  {s.optional && <Badge className="ml-2">optional</Badge>}
                  {c.next === s.key && (
                    <Badge tone="info" className="ml-2">
                      next
                    </Badge>
                  )}
                </span>
                <span className="block truncate text-[12px] text-fg-3">
                  {s.detail}
                </span>
              </span>
              <ChevronRight className="size-4 shrink-0 text-fg-4" />
            </Link>
          </li>
        ))}
      </ol>
    </Panel>
  );
}
