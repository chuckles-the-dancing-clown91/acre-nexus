"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { api, type TenantHistoryRow } from "@/lib/api";
import { Badge, Card, statusTone } from "@/components/ui";
import { useAuth } from "@/lib/auth";

export default function PropertyTenantsPage() {
  const params = useParams<{ id: string }>();
  const { can } = useAuth();
  const [rows, setRows] = useState<TenantHistoryRow[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!can("lease:read")) return;
    api
      .propertyTenantHistory(params.id)
      .then(setRows)
      .catch((e) => setError(e.message));
  }, [params.id, can]);

  if (!can("lease:read")) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          You need the <span className="font-mono">lease:read</span> permission.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <Link
          href={`/console/properties/${params.id}`}
          className="text-sm text-ink-3"
        >
          ← Back to property
        </Link>
        <h1 className="mt-1 font-display text-3xl font-extrabold tracking-tight">
          Tenant history
        </h1>
        <p className="text-ink-3">
          Current and former residents of this property.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      <div className="space-y-3">
        {rows?.map((r) => (
          <Card
            key={`${r.tenant_name}-${r.tenant_email ?? ""}`}
            className="p-4"
          >
            <div className="flex flex-wrap items-center gap-3">
              <div className="flex-1">
                <span className="font-semibold">{r.tenant_name}</span>
                <span className="ml-2 text-sm text-ink-3">
                  {r.tenant_email ?? ""}
                </span>
              </div>
              <Badge tone={r.current ? "good" : "neutral"}>
                {r.current ? "current" : "former"}
              </Badge>
            </div>
            <div className="mt-2 space-y-1">
              {r.tenancies.map((t) => (
                <Link
                  key={t.lease_id}
                  href={`/console/leases/${t.lease_id}`}
                  className="flex flex-wrap items-center gap-3 rounded-md px-2 py-1 text-sm hover:bg-surface-2"
                >
                  <span className="flex-1 text-ink-3">
                    {t.start_date}
                    {t.end_date ? ` → ${t.end_date}` : " → present"}
                  </span>
                  <span className="font-mono">{t.rent_label}/mo</span>
                  <Badge tone={statusTone(t.status)}>{t.status}</Badge>
                </Link>
              ))}
            </div>
          </Card>
        ))}
        {rows && rows.length === 0 && (
          <p className="text-ink-3">No tenants on record for this property.</p>
        )}
      </div>
    </div>
  );
}
