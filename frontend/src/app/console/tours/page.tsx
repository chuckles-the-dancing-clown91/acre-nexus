"use client";

// Tour requests from the public site: who asked, which home, when, and where
// each one stands.

import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { tours, type TourRequest } from "@/lib/tours";
import { useAuth } from "@/lib/auth";
import { Badge, Card } from "@/components/ui";

const STATUSES = ["new", "contacted", "scheduled", "closed"] as const;

export default function ToursPage() {
  const { can } = useAuth();
  const write = can("application:write");
  const [rows, setRows] = useState<TourRequest[]>([]);
  const [status, setStatus] = useState("");
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    tours
      .list(status)
      .then(setRows)
      .catch((e: Error) => setError(e.message));
  }, [status]);
  useEffect(load, [load]);

  const move = async (id: string, next: string) => {
    try {
      await tours.update(id, { status: next });
      load();
    } catch (e) {
      toast.error((e as Error).message);
    }
  };
  const note = async (r: TourRequest) => {
    const n = window.prompt("Note", r.note ?? "");
    if (n === null) return;
    try {
      await tours.update(r.id, { note: n });
      load();
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  return (
    <div className="space-y-5">
      <div>
        <h1 className="font-display text-2xl font-bold">Tour requests</h1>
        <p className="text-sm text-ink-3">
          Prospects who asked to see a home from your website.
        </p>
      </div>
      <div className="flex gap-2">
        {["", ...STATUSES].map((s) => (
          <button
            key={s || "all"}
            onClick={() => setStatus(s)}
            className={`rounded-full px-3 py-1 text-xs font-bold ${
              status === s
                ? "bg-accent-soft text-accent-2"
                : "bg-surface-2 text-ink-2"
            }`}
          >
            {s || "all"}
          </button>
        ))}
      </div>
      {error && <p className="text-sm text-bad">{error}</p>}
      <Card className="divide-y divide-line">
        {rows.map((r) => (
          <div
            key={r.id}
            className="flex flex-wrap items-start gap-4 px-5 py-4"
          >
            <div className="min-w-0 flex-1">
              <div className="font-semibold">
                {r.name}
                <span className="ml-2 text-sm font-normal text-ink-3">
                  {r.listing_title ?? "General"}
                </span>
              </div>
              <div className="text-sm text-ink-2">
                {r.email}
                {r.phone ? ` · ${r.phone}` : ""}
              </div>
              {r.preferred_times && (
                <div className="text-xs text-ink-3">
                  Best times: {r.preferred_times}
                </div>
              )}
              {r.message && (
                <div className="mt-1 text-sm text-ink-2">{r.message}</div>
              )}
              {r.note && (
                <div className="mt-1 text-xs text-ink-3">Note: {r.note}</div>
              )}
            </div>
            <Badge tone={r.status === "new" ? "warn" : "neutral"}>
              {r.status}
            </Badge>
            {write && (
              <div className="flex items-center gap-2">
                <select
                  className="rounded-lg border border-line bg-surface px-2 py-1.5 text-sm"
                  value={r.status}
                  onChange={(e) => move(r.id, e.target.value)}
                  aria-label="Status"
                >
                  {STATUSES.map((s) => (
                    <option key={s}>{s}</option>
                  ))}
                </select>
                <button
                  className="text-xs font-semibold text-accent-2"
                  onClick={() => note(r)}
                >
                  Note
                </button>
              </div>
            )}
          </div>
        ))}
        {rows.length === 0 && !error && (
          <p className="px-5 py-8 text-center text-sm text-ink-3">
            No tour requests.
          </p>
        )}
      </Card>
    </div>
  );
}
