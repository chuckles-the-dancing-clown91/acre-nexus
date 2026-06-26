"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { LlcGroup } from "@/lib/types";
import { Badge, Card, statusTone } from "@/components/ui";

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
              <div className="flex-1">
                <div className="font-display text-lg font-bold">{g.name}</div>
                <div className="text-sm text-ink-3">
                  EIN {g.ein} · {g.state}
                </div>
              </div>
              <div className="text-sm text-ink-2">
                {g.property_count} properties
              </div>
              <div className="text-sm text-ink-2">{g.units} units</div>
              <div className="font-mono text-sm font-bold">
                {g.monthly_rent_label}/mo
              </div>
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
