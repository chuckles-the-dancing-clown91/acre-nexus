"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api";
import type { LlcGroup } from "@/lib/types";
import { Badge, Card, statusTone } from "@/components/ui";
import { Icon } from "@/components/Icon";

export default function LlcsPage() {
  const [groups, setGroups] = useState<LlcGroup[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .llcGroups()
      .then(setGroups)
      .catch((e) => setError(e.message));
  }, []);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-display text-3xl font-extrabold tracking-tight">
          LLCs &amp; properties
        </h1>
        <p className="text-ink-3">
          Your holding entities and the assets they own.
        </p>
      </div>

      {error && <p className="text-bad">{error}</p>}

      <div className="space-y-4">
        {groups?.map((g) => (
          <Card key={g.id} className="overflow-hidden">
            <div className="flex flex-wrap items-center gap-3 border-b border-line px-5 py-4">
              <Link href={`/console/llcs/${g.id}`} className="group flex-1">
                <div className="flex items-center gap-2 font-display text-lg font-bold group-hover:text-accent">
                  {g.name}
                  <Icon
                    name="back"
                    size={16}
                    className="rotate-180 text-ink-3 group-hover:text-accent"
                  />
                </div>
                <div className="text-sm text-ink-3">
                  EIN {g.ein} · {g.state}
                </div>
              </Link>
              <div className="text-sm text-ink-2">
                {g.property_count} properties
              </div>
              <div className="text-sm text-ink-2">{g.units} units</div>
              <div className="font-mono text-sm font-bold">
                {g.monthly_rent_label}/mo
              </div>
              <Link
                href={`/console/llcs/${g.id}`}
                className="rounded-xl border border-line-2 bg-surface px-3 py-1.5 text-sm font-bold text-ink hover:bg-surface-2"
              >
                Onboard
              </Link>
            </div>
            <div className="divide-y divide-line">
              {g.properties.map((p) => (
                <div
                  key={p.id}
                  className="flex items-center gap-4 px-5 py-3 text-sm"
                >
                  <span className="flex-1 font-semibold">{p.name}</span>
                  <span className="text-ink-3">{p.occupancy}</span>
                  <span className="font-mono">{p.monthly_rent_label}</span>
                  <Badge tone={statusTone(p.status)}>{p.status}</Badge>
                </div>
              ))}
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}
