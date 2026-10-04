"use client";

// Workflows: every property moving through its investment strategy's stages.
// Drag a card to another column, or pick a stage from its menu.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { GripVertical, Workflow } from "lucide-react";
import { toast } from "sonner";
import { api, type WorkflowCatalogStage } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { queryKeys, useProperties } from "@/lib/queries";
import type { Property } from "@/lib/types";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Tabs } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const UNSTAGED = "__unstaged__";

export default function WorkflowsPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const ready = scoped && can("property:read");
  const write = can("property:write");
  const qc = useQueryClient();
  const catalog = useQuery({
    queryKey: ["workflow-catalog"],
    queryFn: api.workflowCatalog,
    enabled: ready,
  });
  const properties = useProperties({ enabled: ready });
  const [picked, setPicked] = useState<string | null>(null);
  const [moving, setMoving] = useState<string | null>(null);
  const [dragId, setDragId] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);

  const counts = useMemo(() => {
    const m = new Map<string, number>();
    for (const p of properties.data ?? [])
      m.set(p.strategy, (m.get(p.strategy) ?? 0) + 1);
    return m;
  }, [properties.data]);

  // Start on the strategy with the most properties.
  const active =
    picked ??
    [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0] ??
    catalog.data?.[0]?.key ??
    null;
  const strategy = catalog.data?.find((s) => s.key === active) ?? null;

  const buckets = useMemo(() => {
    const map = new Map<string, Property[]>();
    if (!strategy) return map;
    const known = new Set(strategy.stages.map((s) => s.key));
    for (const p of properties.data ?? []) {
      if (p.strategy !== strategy.key) continue;
      const key = known.has(p.workflow_stage) ? p.workflow_stage : UNSTAGED;
      map.set(key, [...(map.get(key) ?? []), p]);
    }
    return map;
  }, [strategy, properties.data]);

  const columns: WorkflowCatalogStage[] = strategy
    ? (buckets.get(UNSTAGED)?.length ?? 0) > 0
      ? [{ key: UNSTAGED, label: "No stage" }, ...strategy.stages]
      : strategy.stages
    : [];
  const total = counts.get(strategy?.key ?? "") ?? 0;

  async function move(p: Property, to: string) {
    if (to === p.workflow_stage) return;
    setMoving(p.id);
    // Show the move straight away, then settle on what the server says.
    qc.setQueryData<Property[]>(queryKeys.properties, (l) =>
      l?.map((x) => (x.id === p.id ? { ...x, workflow_stage: to } : x))
    );
    try {
      await api.advanceWorkflow(p.id, to);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : "Couldn't move it");
    } finally {
      setMoving(null);
      void qc.invalidateQueries({ queryKey: queryKeys.properties });
    }
  }

  const tabs = (catalog.data ?? []).map(
    (s) => [s.key, `${s.label} · ${counts.get(s.key) ?? 0}`] as const
  );

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Portfolio"
        title="Workflows"
        description="Track every property through its strategy's stages. Drag a card between columns, or use its menu."
      />

      {(catalog.isLoading || properties.isLoading) && (
        <Skeleton className="h-96 rounded-2xl" />
      )}
      {(catalog.error || properties.error) && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load workflows:{" "}
          {(catalog.error ?? properties.error)?.message}
        </Panel>
      )}

      {tabs.length > 0 && active && (
        <div className="space-y-2">
          <Tabs tabs={tabs} value={active} onChange={setPicked} />
          {strategy && (
            <p className="text-[13px] text-fg-3">
              {strategy.description} · {total}{" "}
              {total === 1 ? "property" : "properties"}
            </p>
          )}
        </div>
      )}

      {strategy && total === 0 && (
        <Panel>
          <EmptyState
            icon={<Workflow />}
            title={`No properties on ${strategy.label}`}
            description="A property's strategy is set on its profile."
          />
        </Panel>
      )}

      {strategy && total > 0 && (
        <div className="-mx-1 overflow-x-auto px-1 pb-2">
          <div
            className="grid min-w-full gap-3"
            style={{
              gridTemplateColumns: `repeat(${columns.length}, minmax(13rem, 1fr))`,
            }}
          >
            {columns.map((stage) => {
              const items = buckets.get(stage.key) ?? [];
              const droppable = write && stage.key !== UNSTAGED;
              return (
                <div
                  key={stage.key}
                  className={cn(
                    "space-y-2 rounded-2xl border border-transparent p-1.5 transition-colors",
                    over === stage.key &&
                      droppable &&
                      "border-accent/50 bg-accent/5"
                  )}
                  onDragOver={(e) => {
                    if (!droppable) return;
                    e.preventDefault();
                    if (over !== stage.key) setOver(stage.key);
                  }}
                  onDragLeave={() =>
                    setOver((s) => (s === stage.key ? null : s))
                  }
                  onDrop={(e) => {
                    e.preventDefault();
                    setOver(null);
                    if (!droppable || !dragId) return;
                    const p = properties.data?.find((x) => x.id === dragId);
                    setDragId(null);
                    if (p) void move(p, stage.key);
                  }}
                >
                  <div className="flex items-center justify-between px-1.5 pt-1">
                    <h3 className="text-[13px] font-semibold text-fg">
                      {stage.label}
                    </h3>
                    <span className="figure text-xs text-fg-3">
                      {items.length}
                    </span>
                  </div>
                  {items.map((p) => (
                    <div
                      key={p.id}
                      draggable={write && moving !== p.id}
                      onDragStart={(e) => {
                        setDragId(p.id);
                        e.dataTransfer.effectAllowed = "move";
                      }}
                      onDragEnd={() => {
                        setDragId(null);
                        setOver(null);
                      }}
                      className={cn(
                        write && "cursor-grab active:cursor-grabbing",
                        dragId === p.id && "opacity-50"
                      )}
                    >
                      <Panel className="space-y-2 p-3">
                        <div className="flex items-start gap-1.5">
                          <Link
                            href={`/console/properties/${p.id}`}
                            className="min-w-0 flex-1 text-[13px] leading-tight font-medium text-fg hover:underline"
                          >
                            {p.name}
                          </Link>
                          {write && (
                            <GripVertical className="size-4 shrink-0 text-fg-4" />
                          )}
                        </div>
                        <div className="flex justify-between gap-2 text-xs text-fg-3">
                          <span className="truncate">{p.city}</span>
                          <span className="figure text-fg-2">
                            {p.monthly_rent_label}/mo
                          </span>
                        </div>
                        {write && (
                          <select
                            aria-label={`Move ${p.name} to a stage`}
                            value={
                              stage.key === UNSTAGED ? "" : p.workflow_stage
                            }
                            disabled={moving === p.id}
                            onChange={(e) => void move(p, e.target.value)}
                            className="w-full rounded-lg border border-line bg-surface px-2 py-1 text-xs text-fg-2 disabled:opacity-50"
                          >
                            {stage.key === UNSTAGED && (
                              <option value="" disabled>
                                Move to
                              </option>
                            )}
                            {strategy.stages.map((s) => (
                              <option key={s.key} value={s.key}>
                                {s.label}
                              </option>
                            ))}
                          </select>
                        )}
                      </Panel>
                    </div>
                  ))}
                  {items.length === 0 && (
                    <div className="flex min-h-20 items-center justify-center rounded-xl border border-dashed border-line text-xs text-fg-4">
                      Empty
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
