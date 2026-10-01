"use client";

// The workspace audit trail: every change anyone made, filterable and exportable.

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api";
import { audit, KINDS, type TrailEvent } from "@/lib/audit";
import { download } from "@/lib/backoffice";
import type { Property } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { HistoryList } from "@/components/HistoryList";
import { Button, Card } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

export default function AuditTrailPage() {
  const { can } = useAuth();
  const [properties, setProperties] = useState<Property[]>([]);
  const [propertyId, setPropertyId] = useState("");
  const [kind, setKind] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [support, setSupport] = useState(false);
  const [events, setEvents] = useState<TrailEvent[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const filter = {
    property_id: propertyId,
    target_type: kind,
    from,
    to,
    support,
  };

  const load = useCallback(
    (before?: string) => {
      setLoading(true);
      audit
        .events({
          property_id: propertyId,
          target_type: kind,
          from,
          to,
          support,
          before,
          limit: 50,
        })
        .then((page) => {
          setEvents((cur) => (before ? [...cur, ...page.events] : page.events));
          setNext(page.next);
          setError(null);
        })
        .catch((e: Error) => setError(e.message))
        .finally(() => setLoading(false));
    },
    [propertyId, kind, from, to, support]
  );

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    api
      .properties()
      .then(setProperties)
      .catch(() => setProperties([]));
  }, []);

  if (!can("audit:read"))
    return (
      <p className="text-ink-3">
        You need the audit permission to see the trail.
      </p>
    );

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Audit trail
          </h1>
          <p className="text-ink-3">
            Who changed what, on which property — including Vantedge staff
            helping on a call.
          </p>
        </div>
        <Button
          variant="outline"
          onClick={() =>
            void download(audit.csvPath(filter), "audit-trail.csv")
          }
        >
          Export CSV
        </Button>
      </div>
      <Card className="p-5">
        <div className="mb-4 flex flex-wrap items-center gap-2">
          <select
            className={field}
            value={propertyId}
            onChange={(e) => setPropertyId(e.target.value)}
            aria-label="Property"
          >
            <option value="">All properties</option>
            {properties.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
          <select
            className={field}
            value={kind}
            onChange={(e) => setKind(e.target.value)}
            aria-label="Kind"
          >
            {KINDS.map((k) => (
              <option key={k.value} value={k.value}>
                {k.label}
              </option>
            ))}
          </select>
          <label className="flex items-center gap-1 text-xs text-ink-3">
            from
            <input
              type="date"
              className={field}
              value={from}
              onChange={(e) => setFrom(e.target.value)}
            />
          </label>
          <label className="flex items-center gap-1 text-xs text-ink-3">
            to
            <input
              type="date"
              className={field}
              value={to}
              onChange={(e) => setTo(e.target.value)}
            />
          </label>
          <label className="flex items-center gap-1.5 text-xs text-ink-3">
            <input
              type="checkbox"
              checked={support}
              onChange={(e) => setSupport(e.target.checked)}
            />
            Vantedge support only
          </label>
        </div>
        {error && <p className="text-bad">{error}</p>}
        <HistoryList
          events={events}
          more={!!next}
          onMore={() => next && load(next)}
          loading={loading}
        />
      </Card>
    </div>
  );
}
