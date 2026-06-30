"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api, type TenantHistoryRow } from "@/lib/api";
import { Badge, Card, statusTone } from "@/components/ui";
import { useAuth } from "@/lib/auth";

export default function TenantHistoryPage() {
  const { can } = useAuth();
  const [rows, setRows] = useState<TenantHistoryRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [q, setQ] = useState("");

  useEffect(() => {
    if (!can("lease:read")) return;
    api
      .tenantHistory()
      .then(setRows)
      .catch((e) => setError(e.message));
  }, [can]);

  if (!can("lease:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You don&apos;t have access to tenant history. Ask an admin for the{" "}
          <span className="font-mono">lease:read</span> permission.
        </p>
      </Card>
    );
  }

  const filtered = (rows ?? []).filter((r) => {
    if (!q.trim()) return true;
    const needle = q.toLowerCase();
    return (
      r.tenant_name.toLowerCase().includes(needle) ||
      (r.tenant_email ?? "").toLowerCase().includes(needle)
    );
  });

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="font-display text-3xl font-extrabold tracking-tight">
            Tenant history
          </h1>
          <p className="text-ink-3">
            Every resident the firm has leased to, past and present.
          </p>
        </div>
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search name or email…"
          className="rounded-xl border border-line bg-surface px-3 py-2 text-sm"
        />
      </div>

      {error && <p className="text-bad">{error}</p>}

      <div className="space-y-4">
        {filtered.map((r) => (
          <Card
            key={`${r.tenant_name}-${r.tenant_email ?? ""}`}
            className="overflow-hidden"
          >
            <div className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-4">
              <div className="flex-1">
                <div className="font-display text-lg font-bold">
                  {r.tenant_name}
                </div>
                <div className="text-sm text-ink-3">
                  {r.tenant_email ?? "no email"}
                  {r.tenant_phone ? ` · ${r.tenant_phone}` : ""}
                </div>
              </div>
              <Badge tone={r.current ? "good" : "neutral"}>
                {r.current ? "current resident" : "former"}
              </Badge>
              <span className="text-sm text-ink-2">
                {r.lease_count} {r.lease_count === 1 ? "tenancy" : "tenancies"}
              </span>
            </div>
            <div className="divide-y divide-line">
              {r.tenancies.map((t) => (
                <Link
                  key={t.lease_id}
                  href={`/console/leases/${t.lease_id}`}
                  className="flex flex-wrap items-center gap-4 px-5 py-3 text-sm hover:bg-surface-2"
                >
                  <span className="min-w-[160px] flex-1 font-semibold">
                    {t.property_name ?? "—"}
                  </span>
                  <span className="text-ink-3">
                    {t.start_date}
                    {t.end_date ? ` → ${t.end_date}` : " → present"}
                  </span>
                  <span className="font-mono">{t.rent_label}/mo</span>
                  {t.from_application && <Badge tone="info">applied</Badge>}
                  <Badge tone={statusTone(t.status)}>{t.status}</Badge>
                  {t.balance_cents > 0 && (
                    <Badge tone="bad">owes {t.balance_label}</Badge>
                  )}
                </Link>
              ))}
            </div>
          </Card>
        ))}
        {rows && filtered.length === 0 && (
          <p className="text-ink-3">No tenants found.</p>
        )}
      </div>
    </div>
  );
}
