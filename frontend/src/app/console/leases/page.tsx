"use client";

// Tenants & leases directory: every tenancy with its rental + payment status.
// Gated by `lease:read`.

import { useEffect, useMemo, useState } from "react";
import { api } from "@/lib/api";
import type { Lease, Property } from "@/lib/types";
import { useAuth } from "@/lib/auth";
import { Badge, Card } from "@/components/ui";

const STATUSES = ["active", "upcoming", "notice", "expired", "ended"];

/** Tone for a lease's payment standing. */
function paymentTone(status: string): "good" | "warn" | "bad" | "neutral" {
  if (status === "current") return "good";
  if (status === "partial") return "warn";
  if (status === "late") return "bad";
  return "neutral";
}

export default function LeasesPage() {
  const { can } = useAuth();
  const [leases, setLeases] = useState<Lease[]>([]);
  const [properties, setProperties] = useState<Property[]>([]);
  const [status, setStatus] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!can("lease:read")) return;
    api
      .leases(status ? { status } : {})
      .then(setLeases)
      .catch((e) => setError(e.message));
  }, [status, can]);

  useEffect(() => {
    api
      .properties()
      .then(setProperties)
      .catch(() => {});
  }, []);

  const propName = useMemo(() => {
    const m = new Map(properties.map((p) => [p.id, p.name]));
    return (id: string) => m.get(id) ?? "—";
  }, [properties]);

  if (!can("lease:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to rentals. Ask an admin for the{" "}
          <span className="font-mono">lease:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Tenants &amp; leases
          </h1>
          <p className="text-ink-3">Every tenancy and its payment standing.</p>
        </div>
        <label className="flex flex-col gap-1 text-xs font-semibold text-ink-3">
          Status
          <select
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            className="rounded-xl border border-line bg-surface px-3 py-2 text-sm font-normal text-ink"
          >
            <option value="">All</option>
            {STATUSES.map((s) => (
              <option key={s} value={s}>
                {s}
              </option>
            ))}
          </select>
        </label>
      </div>

      {error && <p className="text-bad">{error}</p>}

      <Card className="overflow-hidden">
        <div className="grid grid-cols-[1.4fr_1.2fr_.8fr_.7fr_.7fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Tenant</span>
          <span>Property</span>
          <span className="text-right">Rent</span>
          <span className="text-right">Status</span>
          <span className="text-right">Payment</span>
        </div>
        <div className="divide-y divide-line">
          {leases.length === 0 ? (
            <div className="px-5 py-10 text-center text-ink-3">No leases.</div>
          ) : (
            leases.map((l) => (
              <div
                key={l.id}
                className="grid grid-cols-[1.4fr_1.2fr_.8fr_.7fr_.7fr] items-center gap-4 px-5 py-3.5"
              >
                <div className="min-w-0">
                  <div className="truncate font-semibold">{l.tenant_name}</div>
                  {l.tenant_email && (
                    <div className="truncate text-sm text-ink-3">
                      {l.tenant_email}
                    </div>
                  )}
                </div>
                <span className="truncate text-sm text-ink-2">
                  {propName(l.property_id)}
                </span>
                <span className="text-right font-mono text-sm">
                  {l.rent_label}
                </span>
                <span className="flex justify-end">
                  <Badge tone="neutral">{l.status}</Badge>
                </span>
                <span className="flex justify-end">
                  <Badge tone={paymentTone(l.payment_status)}>
                    {l.payment_status}
                    {l.balance_cents > 0 ? "" : ""}
                  </Badge>
                </span>
              </div>
            ))
          )}
        </div>
      </Card>
    </div>
  );
}
