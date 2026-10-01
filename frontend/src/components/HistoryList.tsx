"use client";

// A list of audit events: who, when, what, and the before → after of each
// change. Used by the property History tab and the audit trail page.

import { useState } from "react";
import { Badge, Button } from "@/components/ui";
import { fieldLabel, showValue, type TrailEvent } from "@/lib/audit";

function when(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function kindLabel(t: string | null) {
  if (!t) return "";
  return (
    { maintenance_ticket: "work order", site_map: "site map" }[t] ??
    t.replace(/_/g, " ")
  );
}

export function HistoryList({
  events,
  more,
  onMore,
  loading,
}: {
  events: TrailEvent[];
  more: boolean;
  onMore: () => void;
  loading: boolean;
}) {
  const [open, setOpen] = useState<string | null>(null);
  if (events.length === 0 && !loading)
    return (
      <p className="text-sm text-ink-3">
        Nothing has been changed here yet. Edits to the property, its units,
        appliances, work orders, leases and listings show up with who made them.
      </p>
    );
  return (
    <div className="divide-y divide-line text-sm">
      {events.map((e) => {
        const expandable = e.changes.length > 0 || !!e.reason;
        const isOpen = open === e.id;
        return (
          <div key={e.id} className="py-3">
            <button
              type="button"
              className="flex w-full flex-wrap items-start justify-between gap-2 text-left"
              onClick={() => expandable && setOpen(isOpen ? null : e.id)}
              aria-expanded={expandable ? isOpen : undefined}
            >
              <div className="min-w-0">
                <div>
                  <span className="font-semibold">{e.actor_name}</span>{" "}
                  <span className="text-ink-2">{e.summary}</span>{" "}
                  <span className="font-semibold">{e.label}</span>
                  {e.target_type && (
                    <span className="ml-1.5 text-xs text-ink-3">
                      {kindLabel(e.target_type)}
                    </span>
                  )}
                </div>
                <div className="mt-0.5 text-xs text-ink-3">{when(e.at)}</div>
              </div>
              <div className="flex items-center gap-2">
                {e.support && <Badge tone="info">Vantedge support</Badge>}
                {expandable && (
                  <span className="text-xs text-ink-3">
                    {isOpen ? "Hide" : `${e.changes.length || ""} details`}
                  </span>
                )}
              </div>
            </button>
            {isOpen && (
              <div className="mt-2 space-y-1 rounded-xl bg-surface-2 p-3">
                {e.changes.map((c) => (
                  <div
                    key={c.field}
                    className="flex flex-wrap items-baseline gap-x-2"
                  >
                    <span className="w-40 shrink-0 text-ink-3">
                      {fieldLabel(c.field)}
                    </span>
                    <span className="text-ink-3 line-through">
                      {showValue(c.field, c.from)}
                    </span>
                    <span aria-hidden>→</span>
                    <span className="font-semibold">
                      {showValue(c.field, c.to)}
                    </span>
                  </div>
                ))}
                {e.reason && (
                  <div className="text-ink-2">Reason: {e.reason}</div>
                )}
              </div>
            )}
          </div>
        );
      })}
      {more && (
        <div className="pt-3">
          <Button variant="outline" onClick={onMore} disabled={loading}>
            {loading ? "Loading…" : "Show older"}
          </Button>
        </div>
      )}
    </div>
  );
}
