"use client";

// Site maps: every property's layout in one place. Apartments draw buildings
// and units; campgrounds and RV parks draw each site.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import { Map as MapIcon, Plus, Search } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { useProperties } from "@/lib/queries";
import { siteMaps, type SiteMap } from "@/lib/sitemaps";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { fieldClass, Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { F, FormDialog, input } from "@/components/property/bits";
import { BASE_LABEL, MAP_KIND_LABEL } from "./labels";

const rise = (i: number) => ({
  initial: { opacity: 0, y: 10 },
  animate: { opacity: 1, y: 0 },
  transition: {
    duration: 0.45,
    delay: Math.min(i, 12) * 0.035,
    ease: [0.22, 1, 0.36, 1] as const,
  },
});

export default function MapsPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const write = can("property:write");
  const maps = useQuery({
    queryKey: ["site-maps"],
    queryFn: () => siteMaps.list(),
    enabled: scoped && can("property:read"),
  });
  const [property, setProperty] = useState("");
  const [q, setQ] = useState("");
  const [creating, setCreating] = useState(false);
  const properties = useMemo(() => {
    const m = new Map<string, string>();
    for (const x of maps.data ?? []) m.set(x.property_id, x.property_name);
    return [...m.entries()].sort((a, b) => a[1].localeCompare(b[1]));
  }, [maps.data]);
  const rows = useMemo(() => {
    const n = q.trim().toLowerCase();
    return (maps.data ?? []).filter(
      (m) =>
        (!property || m.property_id === property) &&
        (!n ||
          m.name.toLowerCase().includes(n) ||
          m.property_name.toLowerCase().includes(n))
    );
  }, [maps.data, property, q]);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Portfolio"
        title="Site maps"
        description="Lay out a property: buildings and units, or campsites, on satellite, streets, your own plan or a blank grid."
        actions={
          write && (
            <Button onClick={() => setCreating(true)}>
              <Plus />
              New map
            </Button>
          )
        }
      />

      {(maps.data?.length ?? 0) > 3 && (
        <div className="flex flex-wrap gap-3">
          {properties.length > 1 && (
            <select
              aria-label="Property"
              className={fieldClass}
              value={property}
              onChange={(e) => setProperty(e.target.value)}
            >
              <option value="">All properties</option>
              {properties.map(([id, name]) => (
                <option key={id} value={id}>
                  {name}
                </option>
              ))}
            </select>
          )}
          <div className="relative w-full max-w-sm">
            <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
            <Input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="Filter by map or property"
              aria-label="Filter maps"
              className="pl-9"
            />
          </div>
        </div>
      )}

      {maps.isLoading && (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 3 }, (_, i) => (
            <Skeleton key={i} className="h-36 rounded-2xl" />
          ))}
        </div>
      )}
      {maps.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load site maps: {maps.error.message}
        </Panel>
      )}
      {maps.data?.length === 0 && (
        <Panel>
          <EmptyState
            icon={<MapIcon />}
            title="No site maps yet"
            description="Draw a property's buildings and units, or a campground's sites, and show availability on your site."
            action={
              write && (
                <Button onClick={() => setCreating(true)}>
                  <Plus />
                  New map
                </Button>
              )
            }
          />
        </Panel>
      )}
      {maps.data && maps.data.length > 0 && rows.length === 0 && (
        <Panel>
          <EmptyState icon={<Search />} title="Nothing matches" />
        </Panel>
      )}

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
        {rows.map((m, i) => (
          <motion.div key={m.id} {...rise(i)}>
            <MapCard m={m} />
          </motion.div>
        ))}
      </div>

      {write && (
        <NewMap open={creating} onOpenChange={setCreating} enabled={scoped} />
      )}
    </div>
  );
}

function MapCard({ m }: { m: SiteMap }) {
  const apt = m.kind === "apartment";
  const total = apt ? m.stats.units : m.stats.sites;
  const free = apt ? m.stats.units_available : m.stats.sites_available;
  const pct = total ? Math.round((free / total) * 100) : 0;
  return (
    <Link
      href={`/console/maps/${m.id}`}
      className="block rounded-2xl outline-none focus-visible:ring-2 focus-visible:ring-accent"
    >
      <Panel interactive className="space-y-3 p-4">
        <div className="flex items-start gap-3">
          <span className="flex size-10 shrink-0 items-center justify-center rounded-xl border border-line bg-fill text-fg-2">
            <MapIcon className="size-[18px]" />
          </span>
          <div className="min-w-0 flex-1">
            <div className="truncate text-[15px] font-semibold text-fg">
              {m.name}
            </div>
            <div className="truncate text-[13px] text-fg-3">
              {m.property_name}
            </div>
          </div>
          {m.published && <Badge tone="good">published</Badge>}
        </div>
        <div className="flex flex-wrap gap-1.5">
          <Badge>{MAP_KIND_LABEL[m.kind] ?? m.kind}</Badge>
          <Badge>{BASE_LABEL[m.base_layer] ?? m.base_layer}</Badge>
        </div>
        <div>
          <div className="h-1.5 overflow-hidden rounded-full bg-fill">
            <div
              className="h-full rounded-full bg-good"
              style={{ width: `${pct}%` }}
            />
          </div>
          <div className="mt-1.5 text-xs text-fg-3">
            <span className="figure text-fg">{total}</span>{" "}
            {apt ? "units" : "sites"} ·{" "}
            <span className="figure text-fg">{free}</span> available ·{" "}
            {m.stats.features} shapes
          </div>
        </div>
      </Panel>
    </Link>
  );
}

function NewMap({
  open,
  onOpenChange,
  enabled,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  enabled: boolean;
}) {
  const router = useRouter();
  const properties = useProperties({ enabled: enabled && open });
  const [propertyId, setPropertyId] = useState("");
  const [name, setName] = useState("");
  const [kind, setKind] = useState("apartment");
  const [layer, setLayer] = useState("satellite");
  const [busy, setBusy] = useState(false);

  async function create() {
    if (!propertyId) {
      toast.error("Pick a property");
      return;
    }
    setBusy(true);
    try {
      const p = properties.data?.find((x) => x.id === propertyId);
      // Start where the property is, when its address has been located.
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
      toast.error(e instanceof Error ? e.message : "Couldn't make the map");
      setBusy(false);
    }
  }

  return (
    <FormDialog
      open={open}
      onOpenChange={onOpenChange}
      title="New site map"
      description="It starts centred on the property's address when we know where it is."
      busy={busy}
      onSave={create}
    >
      <F label="Property" className="sm:col-span-2">
        <select
          className={input}
          value={propertyId}
          onChange={(e) => setPropertyId(e.target.value)}
          required
        >
          <option value="">Choose a property</option>
          {properties.data?.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </F>
      <F label="Map name" className="sm:col-span-2">
        <input
          className={input}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Leave blank for “Property layout”"
        />
      </F>
      <F label="Kind">
        <select
          className={input}
          value={kind}
          onChange={(e) => setKind(e.target.value)}
        >
          {Object.entries(MAP_KIND_LABEL).map(([k, l]) => (
            <option key={k} value={k}>
              {l}
            </option>
          ))}
        </select>
      </F>
      <F label="Base layer">
        <select
          className={input}
          value={layer}
          onChange={(e) => setLayer(e.target.value)}
        >
          {Object.entries(BASE_LABEL).map(([k, l]) => (
            <option key={k} value={k}>
              {l}
            </option>
          ))}
        </select>
      </F>
    </FormDialog>
  );
}
