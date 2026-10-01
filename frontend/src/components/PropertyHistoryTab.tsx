"use client";

// The property's History tab: everything that touched it, newest first.

import { useCallback, useEffect, useState } from "react";
import { audit, KINDS, type TrailEvent } from "@/lib/audit";
import { HistoryList } from "@/components/HistoryList";
import { Card } from "@/components/ui";

const select =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

export function PropertyHistoryTab({ propertyId }: { propertyId: string }) {
  const [events, setEvents] = useState<TrailEvent[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [kind, setKind] = useState("");
  const [support, setSupport] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    (before?: string) => {
      setLoading(true);
      audit
        .propertyHistory(propertyId, { kind, support, before, limit: 25 })
        .then((page) => {
          setEvents((cur) => (before ? [...cur, ...page.events] : page.events));
          setNext(page.next);
          setError(null);
        })
        .catch((e: Error) => setError(e.message))
        .finally(() => setLoading(false));
    },
    [propertyId, kind, support]
  );

  useEffect(() => {
    load();
  }, [load]);

  return (
    <Card className="p-5">
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <h2 className="font-display text-lg font-bold">Who changed what</h2>
        <div className="flex flex-wrap items-center gap-2">
          <select
            className={select}
            value={kind}
            onChange={(e) => setKind(e.target.value)}
            aria-label="Kind of change"
          >
            {KINDS.map((k) => (
              <option key={k.value} value={k.value}>
                {k.label}
              </option>
            ))}
          </select>
          <label className="flex items-center gap-1.5 text-xs text-ink-3">
            <input
              type="checkbox"
              checked={support}
              onChange={(e) => setSupport(e.target.checked)}
            />
            Vantedge support only
          </label>
        </div>
      </div>
      {error && <p className="text-bad">{error}</p>}
      <HistoryList
        events={events}
        more={!!next}
        onMore={() => next && load(next)}
        loading={loading}
      />
    </Card>
  );
}
