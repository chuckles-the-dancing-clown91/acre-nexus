"use client";

import { useEffect, useState } from "react";
import {
  api,
  type Observability,
  type PlatformMetrics,
  type TenantSummary,
} from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { Badge, Card, StatTile, statusTone } from "@/components/ui";

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400)
    return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
  return `${Math.floor(secs / 86400)}d ${Math.floor((secs % 86400) / 3600)}h`;
}

export default function PlatformPage() {
  const { user } = useAuth();
  const [metrics, setMetrics] = useState<PlatformMetrics | null>(null);
  const [tenants, setTenants] = useState<TenantSummary[] | null>(null);
  const [health, setHealth] = useState<Observability | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!user?.is_platform_staff) return;
    Promise.all([api.platformMetrics(), api.platformTenants()])
      .then(([m, t]) => {
        setMetrics(m);
        setTenants(t);
      })
      .catch((e) => setError(e.message));
    // System health is best-effort — don't block the page on it.
    api
      .platformObservability()
      .then(setHealth)
      .catch(() => setHealth(null));
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
        <p className="text-ink-3">
          Acre HQ — every client company on the platform.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      {metrics && (
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <StatTile
            label="Tenants"
            value={`${metrics.tenant_count}`}
            icon="globe"
          />
          <StatTile
            label="Active"
            value={`${metrics.active_tenants}`}
            icon="check"
          />
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

      {health && (
        <Card className="p-5">
          <div className="mb-3 flex items-center justify-between">
            <div className="font-display text-lg font-bold">System health</div>
            <Badge tone={health.server_errors > 0 ? "warn" : "good"}>
              {health.server_errors > 0
                ? `${health.server_errors} server errors`
                : "healthy"}
            </Badge>
          </div>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-6">
            <StatTile
              label="Uptime"
              value={formatUptime(health.uptime_secs)}
              icon="calendar"
            />
            <StatTile
              label="Requests"
              value={health.total_requests.toLocaleString()}
              icon="chart"
            />
            <StatTile
              label="Avg latency"
              value={`${health.avg_latency_ms} ms`}
              icon="bell"
            />
            <StatTile
              label="In flight"
              value={`${health.in_flight}`}
              icon="globe"
            />
            <StatTile
              label="Jobs pending"
              value={`${health.jobs.pending ?? 0}`}
              icon="wrench"
            />
            <StatTile
              label="Jobs failed"
              value={`${health.jobs.failed ?? 0}`}
              icon="shield"
            />
          </div>
        </Card>
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
