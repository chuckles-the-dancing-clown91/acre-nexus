"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api, actingTenant } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useFinanceSeries } from "@/lib/queries";
import { bpsLabel, compactUsd } from "@/lib/chart";
import type { PortfolioSummary, Property } from "@/lib/types";
import { Badge, Card, StatTile, statusTone } from "@/components/ui";
import { TrendChart } from "@/components/TrendChart";

export default function DashboardPage() {
  const { user } = useAuth();
  const [needsTenant, setNeedsTenant] = useState(false);
  const [summary, setSummary] = useState<PortfolioSummary | null>(null);
  const [properties, setProperties] = useState<Property[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  function load() {
    setError(null);
    Promise.all([api.portfolioSummary(), api.properties()])
      .then(([s, p]) => {
        setSummary(s);
        setProperties(p);
      })
      .catch((e) => setError(e.message));
  }

  useEffect(() => {
    if (user?.is_platform_staff && !actingTenant.get()) {
      setNeedsTenant(true);
      return;
    }
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [user]);

  if (needsTenant) {
    return <TenantPicker onPick={() => (setNeedsTenant(false), load())} />;
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Portfolio
        </h1>
        <p className="text-ink-3">Live KPIs across your managed properties.</p>
      </div>

      {error && (
        <p className="text-bad">Couldn&apos;t load portfolio: {error}</p>
      )}

      {summary && (
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          {summary.kpis.map((k) => (
            <StatTile
              key={k.label}
              label={k.label}
              value={k.value}
              icon="dollar"
            />
          ))}
        </div>
      )}

      {!needsTenant && <Trends />}

      <Card className="overflow-hidden">
        <div className="border-b border-line px-5 py-4 font-display text-lg font-bold">
          Properties
        </div>
        <div className="divide-y divide-line">
          {properties?.map((p) => (
            <Link
              key={p.id}
              href={`/console/properties/${p.id}`}
              className="flex items-center gap-4 px-5 py-3.5 hover:bg-surface-2"
            >
              <div className="min-w-0 flex-1">
                <div className="truncate font-semibold">{p.name}</div>
                <div className="truncate text-sm text-ink-3">
                  {p.address} · {p.city}
                </div>
              </div>
              <div className="hidden w-24 text-sm text-ink-2 sm:block">
                {p.occupancy} occ.
              </div>
              <div className="hidden w-28 text-right font-mono text-sm sm:block">
                {p.monthly_rent_label}/mo
              </div>
              <Badge tone={statusTone(p.status)}>{p.status}</Badge>
            </Link>
          ))}
          {properties && properties.length === 0 && (
            <div className="px-5 py-10 text-center text-ink-3">
              No properties yet.
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}

/** Twelve months of financial trends, from ledger + snapshot history. */
function Trends() {
  const { can } = useAuth();
  const { data: series } = useFinanceSeries(12, {
    enabled: can("ledger:read"),
  });
  if (!can("ledger:read") || !series || series.months.length === 0) return null;
  return (
    <div className="grid gap-3 lg:grid-cols-2 xl:grid-cols-3">
      <TrendChart
        title="Rent collected"
        months={series.months}
        values={series.rent_collected_cents}
        kind="bar"
        format={compactUsd}
      />
      <TrendChart
        title="Occupancy"
        months={series.months}
        values={series.occupancy_bps}
        format={bpsLabel}
        color="var(--good)"
      />
      <TrendChart
        title="Portfolio value"
        months={series.months}
        values={series.portfolio_value_cents}
        format={compactUsd}
      />
      <TrendChart
        title="NOI"
        months={series.months}
        values={series.noi_cents}
        kind="bar"
        format={compactUsd}
        color="var(--info)"
      />
      <TrendChart
        title="Delinquency"
        months={series.months}
        values={series.delinquency_bps}
        format={bpsLabel}
        color="var(--bad)"
      />
    </div>
  );
}

function TenantPicker({ onPick }: { onPick: () => void }) {
  const tenants = [
    { slug: "northwind", name: "Northwind Property Group" },
    { slug: "cascade", name: "Cascade Living LLC" },
  ];
  return (
    <Card className="mx-auto max-w-md p-6">
      <h2 className="mb-1 font-display text-xl font-bold">View as client</h2>
      <p className="mb-4 text-sm text-ink-3">
        You&apos;re signed in as platform staff. Pick a client workspace to
        inspect (sends <code>X-Tenant</code> on requests).
      </p>
      <div className="space-y-2">
        {tenants.map((t) => (
          <button
            key={t.slug}
            onClick={() => {
              actingTenant.set(t.slug);
              onPick();
            }}
            className="block w-full rounded-xl border border-line bg-surface-2 px-4 py-3 text-left font-semibold hover:border-accent"
          >
            {t.name}
          </button>
        ))}
      </div>
    </Card>
  );
}
