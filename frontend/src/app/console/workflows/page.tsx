"use client";

import { useEffect, useMemo, useState } from "react";
import {
  api,
  type WorkflowStrategy,
  type WorkflowCatalogStage,
} from "@/lib/api";
import type { Property } from "@/lib/types";
import { Card } from "@/components/ui";

const UNSTAGED = "__unstaged__";

export default function WorkflowsPage() {
  const [catalog, setCatalog] = useState<WorkflowStrategy[] | null>(null);
  const [properties, setProperties] = useState<Property[] | null>(null);
  const [active, setActive] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [moving, setMoving] = useState<string | null>(null);

  const load = () =>
    Promise.all([api.workflowCatalog(), api.properties()])
      .then(([c, p]) => {
        setCatalog(c);
        setProperties(p);
        // Default to the strategy with the most properties, else the first.
        setActive((prev) => {
          if (prev) return prev;
          const counts = new Map<string, number>();
          p.forEach((pr) =>
            counts.set(pr.strategy, (counts.get(pr.strategy) ?? 0) + 1)
          );
          const best = [...counts.entries()].sort((a, b) => b[1] - a[1])[0];
          return best?.[0] ?? c[0]?.key ?? null;
        });
      })
      .catch((e) => setError(e.message));

  useEffect(() => {
    load();
  }, []);

  const strategy = useMemo(
    () => catalog?.find((s) => s.key === active) ?? null,
    [catalog, active]
  );

  // Properties on the active strategy, bucketed by stage key.
  const buckets = useMemo(() => {
    const map = new Map<string, Property[]>();
    if (!strategy || !properties) return map;
    const known = new Set(strategy.stages.map((s) => s.key));
    for (const p of properties) {
      if (p.strategy !== strategy.key) continue;
      const key = known.has(p.workflow_stage) ? p.workflow_stage : UNSTAGED;
      const arr = map.get(key) ?? [];
      arr.push(p);
      map.set(key, arr);
    }
    return map;
  }, [strategy, properties]);

  const hasUnstaged = (buckets.get(UNSTAGED)?.length ?? 0) > 0;

  async function move(p: Property, toStage: string) {
    if (toStage === p.workflow_stage) return;
    setMoving(p.id);
    setError(null);
    try {
      await api.advanceWorkflow(p.id, toStage);
      // Optimistically reflect the move, then refresh from the server.
      setProperties(
        (prev) =>
          prev?.map((x) =>
            x.id === p.id ? { ...x, workflow_stage: toStage } : x
          ) ?? prev
      );
      await load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setMoving(null);
    }
  }

  const columns: WorkflowCatalogStage[] = strategy
    ? hasUnstaged
      ? [{ key: UNSTAGED, label: "Unstaged" }, ...strategy.stages]
      : strategy.stages
    : [];

  const total =
    properties?.filter((p) => p.strategy === strategy?.key).length ?? 0;

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Workflows
        </h1>
        <p className="text-ink-3">
          Track every property through its investment strategy&apos;s stages.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {/* Strategy tabs */}
      <div className="flex flex-wrap gap-2">
        {catalog?.map((s) => {
          const count =
            properties?.filter((p) => p.strategy === s.key).length ?? 0;
          const on = s.key === active;
          return (
            <button
              key={s.key}
              onClick={() => setActive(s.key)}
              className={`rounded-lg border px-3 py-1.5 text-sm font-semibold ${
                on
                  ? "border-accent bg-accent text-white"
                  : "border-line text-ink-2 hover:border-ink-3"
              }`}
            >
              {s.label}
              <span className={`ml-2 ${on ? "text-white/80" : "text-ink-3"}`}>
                {count}
              </span>
            </button>
          );
        })}
      </div>

      {strategy && (
        <p className="text-sm text-ink-3">
          {strategy.description} · {total}{" "}
          {total === 1 ? "property" : "properties"}
        </p>
      )}

      {/* Board */}
      {strategy && (
        <div
          className="grid grid-cols-1 gap-3 sm:grid-cols-2"
          style={{
            gridTemplateColumns: `repeat(${Math.min(columns.length, 6)}, minmax(0, 1fr))`,
          }}
        >
          {columns.map((stage) => {
            const items = buckets.get(stage.key) ?? [];
            return (
              <div key={stage.key} className="space-y-2">
                <div className="flex items-center justify-between px-1">
                  <h3 className="font-display text-sm font-bold">
                    {stage.label}
                  </h3>
                  <span className="text-xs text-ink-3">{items.length}</span>
                </div>
                <div className="space-y-2">
                  {items.map((p) => (
                    <Card key={p.id} className="space-y-2 p-3">
                      <div className="font-semibold leading-tight">
                        {p.name}
                      </div>
                      <div className="text-xs text-ink-3">{p.city}</div>
                      <div className="font-mono text-xs">
                        {p.monthly_rent_label}/mo
                      </div>
                      <select
                        aria-label={`Move ${p.name} to a stage`}
                        value={stage.key === UNSTAGED ? "" : p.workflow_stage}
                        disabled={moving === p.id}
                        onChange={(e) => move(p, e.target.value)}
                        className="w-full rounded-md border border-line bg-surface px-2 py-1 text-xs disabled:opacity-50"
                      >
                        {stage.key === UNSTAGED && (
                          <option value="" disabled>
                            Move to…
                          </option>
                        )}
                        {strategy.stages.map((s) => (
                          <option key={s.key} value={s.key}>
                            {s.label}
                          </option>
                        ))}
                      </select>
                    </Card>
                  ))}
                  {items.length === 0 && (
                    <Card className="flex min-h-20 items-center justify-center p-3 text-center text-xs text-ink-3">
                      —
                    </Card>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
