"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { api } from "@/lib/api";
import type { PropertyProfile } from "@/lib/types";
import { Badge, Card, StatTile, statusTone } from "@/components/ui";
import { Icon } from "@/components/Icon";

export default function PropertyProfilePage() {
  const params = useParams<{ id: string }>();
  const [p, setP] = useState<PropertyProfile | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!params.id) return;
    api.property(params.id).then(setP).catch((e) => setError(e.message));
  }, [params.id]);

  if (error) return <p className="text-bad">Couldn&apos;t load property: {error}</p>;
  if (!p) return <p className="text-ink-3">Loading…</p>;

  return (
    <div className="space-y-6">
      <Link
        href="/console/properties"
        className="inline-flex items-center gap-2 text-sm font-semibold text-ink-2"
      >
        <Icon name="back" size={16} /> All properties
      </Link>

      <div className="flex flex-wrap items-center gap-3">
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          {p.name}
        </h1>
        <Badge tone={statusTone(p.status)}>{p.status}</Badge>
      </div>
      <p className="-mt-3 text-ink-3">
        {p.address} · {p.city}
      </p>

      {/* KPI row */}
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        {p.kpis.map((k) => (
          <StatTile key={k.label} label={k.label} value={k.amount_label} icon="dollar" />
        ))}
      </div>

      <div className="grid gap-6 lg:grid-cols-[1.4fr_1fr]">
        {/* Cost breakdown */}
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">
            Monthly cost &amp; revenue
          </h2>
          <div className="space-y-2.5">
            {p.cost_breakdown.map((line) => (
              <div
                key={line.label}
                className="flex items-center justify-between border-b border-line pb-2.5 last:border-0"
              >
                <span className="text-sm text-ink-2">{line.label}</span>
                <span
                  className={`font-mono text-sm ${
                    line.amount_cents >= 0 ? "text-good" : "text-ink"
                  }`}
                >
                  {line.amount_cents >= 0 ? "+" : "−"}
                  {line.amount_label.replace("-", "")}
                </span>
              </div>
            ))}
            <div className="flex items-center justify-between pt-1">
              <span className="font-bold">Net revenue</span>
              <span className="font-mono text-lg font-bold text-good">
                {p.net_revenue_label}
              </span>
            </div>
          </div>
        </Card>

        {/* Details */}
        <Card className="p-5">
          <h2 className="mb-4 font-display text-lg font-bold">Details</h2>
          <dl className="space-y-3 text-sm">
            <Row k="Units" v={`${p.units}`} />
            <Row k="Occupancy" v={p.occupancy} />
            <Row k="Year built" v={`${p.year_built}`} />
            <Row k="Manager" v={p.manager} />
            <Row k="Monthly rent" v={`${p.monthly_rent_label}/mo`} />
          </dl>
        </Card>
      </div>
    </div>
  );
}

function Row({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex items-center justify-between border-b border-line pb-2.5 last:border-0">
      <dt className="text-ink-3">{k}</dt>
      <dd className="font-semibold">{v}</dd>
    </div>
  );
}
