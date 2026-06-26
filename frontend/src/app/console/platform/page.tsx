"use client";

import { useEffect, useState } from "react";
import { api, type PlatformMetrics, type TenantSummary } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, Card, StatTile, statusTone } from "@/components/ui";

export default function PlatformPage() {
  const { user } = useAuth();
  const [metrics, setMetrics] = useState<PlatformMetrics | null>(null);
  const [tenants, setTenants] = useState<TenantSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!user?.is_platform_staff) return;
    Promise.all([api.platformMetrics(), api.platformTenants()])
      .then(([m, t]) => {
        setMetrics(m);
        setTenants(t);
      })
      .catch((e) => setError(e.message));
  }, [user]);

  if (!user?.is_platform_staff) {
    return (
      <Card className="p-6">
        <p className="text-ink-2">
          This is the platform (Acre HQ) admin — staff only. Client workspaces
          can&apos;t see it.
        </p>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Platform admin
        </h1>
        <p className="text-ink-3">Acre HQ — every client company on the platform.</p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {metrics && (
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <StatTile label="Tenants" value={`${metrics.tenant_count}`} icon="globe" />
          <StatTile label="Active" value={`${metrics.active_tenants}`} icon="check" />
          <StatTile
            label="Properties"
            value={`${metrics.total_properties}`}
            icon="building"
          />
          <StatTile
            label="Managed rent"
            value={metrics.total_managed_revenue_label}
            icon="dollar"
          />
        </div>
      )}

      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Client companies
        </div>
        <div className="divide-y divide-line">
          {tenants?.map((t) => (
            <div key={t.id} className="flex items-center gap-4 px-5 py-3.5">
              <div className="min-w-0 flex-1">
                <div className="font-semibold">{t.name}</div>
                <div className="text-sm text-ink-3">
                  {t.slug} · {t.property_count} properties
                </div>
              </div>
              <Badge tone="info">{t.plan}</Badge>
              <Badge tone={statusTone(t.status)}>{t.status}</Badge>
              <div className="hidden w-28 text-right font-mono text-sm sm:block">
                {t.managed_revenue_label}
              </div>
            </div>
          ))}
        </div>
      </Card>
    </div>
  );
}
