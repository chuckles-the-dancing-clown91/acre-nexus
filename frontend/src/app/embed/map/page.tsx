"use client";

// An embeddable site map: a published map, read-only, with a click-to-see
// popover for each unit or site.

import { useSearchParams } from "next/navigation";
import { Suspense, useEffect, useState } from "react";
import { EmbedFrame } from "@/components/embed/EmbedFrame";
import { SiteMapCanvas } from "@/components/maps";
import {
  fromServer,
  siteMaps,
  type Feature,
  type SiteMap,
} from "@/lib/sitemaps";

function View({ tenant }: { tenant: string }) {
  const id = useSearchParams().get("map") ?? "";
  const [map, setMap] = useState<SiteMap | null>(null);
  const [features, setFeatures] = useState<Feature[]>([]);
  const [planUrl, setPlanUrl] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) {
      setError("Add data-map with the map's id.");
      return;
    }
    siteMaps
      .publicGet(id, tenant)
      .then((m) => {
        setMap(m);
        setFeatures(fromServer(m));
        if (m.base_layer === "plan")
          siteMaps
            .publicPlan(id, tenant)
            .then((p) => setPlanUrl(p.url))
            .catch(() => {});
      })
      .catch(() => setError("This map is not available."));
  }, [id, tenant]);

  if (error) return <p className="text-sm text-ink-3">{error}</p>;
  if (!map) return <p className="text-sm text-ink-3">Loading…</p>;
  const sel = features.find((f) => f.key === selected);

  return (
    <div className="space-y-2">
      <div className="h-[420px] overflow-hidden rounded-2xl border border-line">
        <SiteMapCanvas
          map={map}
          planUrl={planUrl}
          features={features}
          selectedKey={selected}
          tool={{ mode: "select" }}
          editKey={null}
          onSelect={setSelected}
          onDrawn={() => {}}
          onEdited={() => {}}
        />
      </div>
      {sel && (
        <div className="rounded-xl border border-line bg-surface p-3 text-sm">
          <div className="font-semibold">{sel.name ?? sel.kind}</div>
          {sel.unit && (
            <div className="text-ink-2">
              {sel.unit.status === "vacant" ? "Available" : "Not available"}
              {sel.unit.market_rent_label
                ? ` · ${sel.unit.market_rent_label}`
                : ""}
            </div>
          )}
          {sel.kind === "site" && (
            <div className="text-ink-2">
              {String(sel.attrs.site_type ?? "")}
              {typeof sel.attrs.rate_cents_night === "number"
                ? ` · $${(sel.attrs.rate_cents_night / 100).toFixed(0)}/night`
                : ""}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function EmbedMap() {
  return (
    <EmbedFrame>
      {({ tenant }) => (
        <Suspense fallback={null}>
          <View tenant={tenant} />
        </Suspense>
      )}
    </EmbedFrame>
  );
}
