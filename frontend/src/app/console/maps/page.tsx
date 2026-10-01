"use client";

// Site maps: every property's layout in one place. Apartments draw buildings
// and units; campgrounds draw each site.

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { Property } from "@/lib/types";
import { siteMaps, type SiteMap } from "@/lib/sitemaps";
import { useAuth } from "@/lib/auth";
import { Badge, Button, Card } from "@/components/ui";

const field =
  "rounded-xl border border-line bg-surface px-3 py-2 text-sm text-ink";

const KIND_LABEL: Record<string, string> = {
  apartment: "Apartments",
  campground: "Campground",
  rv_park: "RV park",
  other: "Other",
};

export default function MapsPage() {
  const { can } = useAuth();
  const write = can("property:write");
  const [maps, setMaps] = useState<SiteMap[]>([]);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    siteMaps
      .list()
      .then(setMaps)
      .catch((e: Error) => setError(e.message));
  }, []);
  useEffect(load, [load]);

  return (
    <div className="space-y-5">
      <div>
        <h1 className="font-display text-2xl font-bold">Site maps</h1>
        <p className="text-sm text-ink-3">
          Lay out a property: buildings and units, or campsites on a custom map.
        </p>
      </div>
      {write && <NewMap />}
      {error && <p className="text-sm text-bad">{error}</p>}
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {maps.map((m) => (
          <Link key={m.id} href={`/console/maps/${m.id}`}>
            <Card className="space-y-2 p-4 hover:border-accent">
              <div className="flex items-start gap-2">
                <div className="min-w-0 flex-1">
                  <div className="font-semibold">{m.name}</div>
                  <div className="text-xs text-ink-3">{m.property_name}</div>
                </div>
                <Badge>{KIND_LABEL[m.kind] ?? m.kind}</Badge>
              </div>
              <div className="text-sm text-ink-2">
                {m.kind === "apartment"
                  ? `${m.stats.units} units, ${m.stats.units_available} available`
                  : `${m.stats.sites} sites, ${m.stats.sites_available} available`}
              </div>
              {m.published && <Badge tone="good">Published</Badge>}
            </Card>
          </Link>
        ))}
        {maps.length === 0 && !error && (
          <p className="text-sm text-ink-3">No maps yet.</p>
        )}
      </div>
    </div>
  );
}

function NewMap() {
  const router = useRouter();
  const [properties, setProperties] = useState<Property[]>([]);
  const [propertyId, setPropertyId] = useState("");
  const [name, setName] = useState("");
  const [kind, setKind] = useState("apartment");
  const [layer, setLayer] = useState("satellite");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .properties()
      .then(setProperties)
      .catch(() => {});
  }, []);

  const create = async () => {
    setBusy(true);
    try {
      const p = properties.find((x) => x.id === propertyId);
      // Start where the property is: its geocoded address, when known.
      const at = await api
        .property(propertyId)
        .then((x) => x.address_status)
        .catch(() => null);
      const m = await siteMaps.create({
        property_id: propertyId,
        name: name.trim() || `${p?.name ?? "Property"} layout`,
        kind,
        base_layer: layer,
        center_lng: at?.longitude ?? undefined,
        center_lat: at?.latitude ?? undefined,
      });
      router.push(`/console/maps/${m.id}`);
    } catch (e) {
      toast.error((e as Error).message);
      setBusy(false);
    }
  };

  return (
    <Card className="flex flex-wrap items-end gap-3 p-4">
      <select
        className={field}
        value={propertyId}
        onChange={(e) => setPropertyId(e.target.value)}
        aria-label="Property"
      >
        <option value="">Property…</option>
        {properties.map((p) => (
          <option key={p.id} value={p.id}>
            {p.name}
          </option>
        ))}
      </select>
      <input
        className={field}
        placeholder="Map name"
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <select
        className={field}
        value={kind}
        onChange={(e) => setKind(e.target.value)}
        aria-label="Kind"
      >
        {Object.entries(KIND_LABEL).map(([k, l]) => (
          <option key={k} value={k}>
            {l}
          </option>
        ))}
      </select>
      <select
        className={field}
        value={layer}
        onChange={(e) => setLayer(e.target.value)}
        aria-label="Base layer"
      >
        <option value="satellite">Satellite</option>
        <option value="streets">Streets</option>
        <option value="plan">My plan image</option>
        <option value="grid">Blank</option>
      </select>
      <Button onClick={create} disabled={!propertyId || busy}>
        Create map
      </Button>
    </Card>
  );
}
