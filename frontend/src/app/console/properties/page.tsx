"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import type { Property } from "@/lib/types";
import { Badge, Card, statusTone } from "@/components/ui";

export default function PropertiesPage() {
  const [properties, setProperties] = useState<Property[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.properties().then(setProperties).catch((e) => setError(e.message));
  }, []);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          Properties
        </h1>
        <p className="text-ink-3">Every asset in your portfolio.</p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      <Card className="overflow-hidden">
        <div className="grid grid-cols-[1.5fr_.7fr_.8fr_.6fr] gap-4 border-b border-line px-5 py-3 text-xs font-bold uppercase tracking-wide text-ink-3">
          <span>Property</span>
          <span>Occupancy</span>
          <span className="text-right">Rent</span>
          <span className="text-right">Status</span>
        </div>
        <div className="divide-y divide-line">
          {properties?.map((p) => (
            <Link
              key={p.id}
              href={`/console/properties/${p.id}`}
              className="grid grid-cols-[1.5fr_.7fr_.8fr_.6fr] items-center gap-4 px-5 py-3.5 hover:bg-surface-2"
            >
              <div className="min-w-0">
                <div className="truncate font-semibold">{p.name}</div>
                <div className="truncate text-sm text-ink-3">{p.city}</div>
              </div>
              <span className="text-sm text-ink-2">{p.occupancy}</span>
              <span className="text-right font-mono text-sm">
                {p.monthly_rent_label}
              </span>
              <span className="flex justify-end">
                <Badge tone={statusTone(p.status)}>{p.status}</Badge>
              </span>
            </Link>
          ))}
        </div>
      </Card>
    </div>
  );
}
