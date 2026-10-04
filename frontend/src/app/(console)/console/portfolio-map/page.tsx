"use client";

// Every property on one map, coloured by occupancy or by open work, with the
// site map and the property one click away.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { ChevronRight, MapPinOff } from "lucide-react";
import { analytics, pinTone, type MapPin, type PinMode } from "@/lib/analytics";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { TileMap } from "@/components/map/TileMap";
import { Badge } from "@/components/ui/badge";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { usd } from "@/lib/format";
import { cn } from "@/lib/utils";

const LEGEND: Record<PinMode, [string, string][]> = {
  occupancy: [
    ["bg-good", "95% or more"],
    ["bg-warn", "85 to 94%"],
    ["bg-bad", "Under 85%"],
    ["bg-fg-3", "No units"],
  ],
  work: [
    ["bg-good", "Nothing open"],
    ["bg-warn", "Work or a turn open"],
    ["bg-bad", "Urgent work open"],
  ],
};

export default function PortfolioMapPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const q = useQuery({
    queryKey: ["portfolio", "map"],
    queryFn: analytics.map,
    enabled: scoped && can("property:read"),
  });
  const [mode, setMode] = useState<PinMode>("occupancy");
  const [sel, setSel] = useState<string | null>(null);
  const all = useMemo(() => q.data?.pins ?? [], [q.data]);
  const placed = all.filter((p) => p.lat !== null && p.lng !== null);
  const pins = placed.map((p) => ({
    id: p.property_id,
    lat: p.lat as number,
    lng: p.lng as number,
    label: `${p.name}, ${p.address}`,
    tone: pinTone(p, mode),
  }));
  const chosen = all.find((p) => p.property_id === sel) ?? null;
  const units = all.reduce((s, p) => s + p.units, 0);
  const occupied = all.reduce((s, p) => s + p.occupied, 0);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Portfolio"
        title="Portfolio map"
        description="Every property you see, on one map."
      />
      {q.isLoading && <Skeleton className="h-[460px] rounded-2xl" />}
      {q.data && all.length === 0 && (
        <EmptyState
          title="No properties yet"
          description="Add one and it shows up here."
        />
      )}
      {q.data && all.length > 0 && (
        <>
          <div className="flex flex-wrap items-center gap-3">
            <div role="tablist" className="flex rounded-xl bg-surface-2 p-1">
              {(["occupancy", "work"] as const).map((m) => (
                <button
                  key={m}
                  role="tab"
                  type="button"
                  aria-selected={mode === m}
                  onClick={() => setMode(m)}
                  className={cn(
                    "rounded-lg px-4 py-1.5 text-[13px] font-medium transition",
                    mode === m
                      ? "bg-surface text-fg shadow-sm"
                      : "text-fg-3 hover:text-fg"
                  )}
                >
                  {m === "occupancy" ? "Occupancy" : "Open work"}
                </button>
              ))}
            </div>
            <div className="flex flex-wrap gap-3 text-[12px] text-fg-3">
              {LEGEND[mode].map(([c, w]) => (
                <span key={w} className="flex items-center gap-1.5">
                  <span className={cn("size-2.5 rounded-full", c)} />
                  {w}
                </span>
              ))}
            </div>
            <div className="ml-auto text-[12px] text-fg-3">
              {all.length} properties · {occupied} of {units} units occupied
            </div>
          </div>
          <div className="grid gap-6 lg:grid-cols-[1fr_340px]">
            <div>
              {pins.length > 0 ? (
                <TileMap pins={pins} selected={sel} onSelect={setSel} />
              ) : (
                <EmptyState
                  title="Nothing to place yet"
                  description="Refresh a property's data to look up where it is."
                />
              )}
              {q.data.unplaced > 0 && (
                <p className="mt-2 flex items-center gap-1.5 text-[12px] text-fg-3">
                  <MapPinOff className="size-3.5" />
                  {q.data.unplaced} not on the map yet. Refresh their property
                  data to place them.
                </p>
              )}
            </div>
            <Panel>
              <PanelHeader
                title={chosen ? chosen.name : "Properties"}
                description={
                  chosen
                    ? `${chosen.address}, ${chosen.city}`
                    : "Pick a pin or a row."
                }
              />
              {chosen ? (
                <Detail p={chosen} onBack={() => setSel(null)} />
              ) : (
                <ul className="max-h-[400px] divide-y divide-line overflow-y-auto">
                  {all.map((p) => (
                    <li key={p.property_id}>
                      <button
                        type="button"
                        onClick={() => setSel(p.property_id)}
                        className="flex w-full items-center gap-3 px-5 py-2.5 text-left hover:bg-surface-2"
                      >
                        <span
                          className={cn(
                            "size-2.5 shrink-0 rounded-full",
                            {
                              good: "bg-good",
                              warn: "bg-warn",
                              bad: "bg-bad",
                              neutral: "bg-fg-3",
                            }[pinTone(p, mode)]
                          )}
                        />
                        <span className="min-w-0 flex-1">
                          <span className="block truncate text-[13px] font-medium text-fg">
                            {p.name}
                          </span>
                          <span className="block truncate text-[12px] text-fg-3">
                            {p.occupancy_pct !== null
                              ? `${p.occupancy_pct}% occupied`
                              : "No units"}{" "}
                            · {p.open_tickets} open
                          </span>
                        </span>
                        <ChevronRight className="size-4 text-fg-3" />
                      </button>
                    </li>
                  ))}
                </ul>
              )}
            </Panel>
          </div>
        </>
      )}
    </div>
  );
}

function Detail({ p, onBack }: { p: MapPin; onBack: () => void }) {
  return (
    <div className="space-y-4 px-5 pb-5">
      <dl className="grid grid-cols-2 gap-3 text-[13px]">
        <div>
          <dt className="text-fg-3">Occupied</dt>
          <dd className="font-medium text-fg">
            {p.occupied} of {p.units}
            {p.occupancy_pct !== null && ` (${p.occupancy_pct}%)`}
          </dd>
        </div>
        <div>
          <dt className="text-fg-3">Rent a month</dt>
          <dd className="font-medium text-fg">{usd(p.monthly_rent_cents)}</dd>
        </div>
        <div>
          <dt className="text-fg-3">Open work orders</dt>
          <dd className="flex items-center gap-1.5 font-medium text-fg">
            {p.open_tickets}
            {p.urgent_tickets > 0 && (
              <Badge tone="bad">{p.urgent_tickets} urgent</Badge>
            )}
          </dd>
        </div>
        <div>
          <dt className="text-fg-3">Turns open</dt>
          <dd className="font-medium text-fg">{p.open_turns}</dd>
        </div>
      </dl>
      <div className="flex flex-wrap gap-2 text-[13px]">
        <Link
          href={`/console/properties/${p.property_id}`}
          className="font-medium text-accent hover:underline"
        >
          Open the property
        </Link>
        {p.site_map_id && (
          <Link
            href={`/console/maps/${p.site_map_id}`}
            className="font-medium text-accent hover:underline"
          >
            Site map
          </Link>
        )}
        <Link
          href={`/console/maintenance?property_id=${p.property_id}`}
          className="font-medium text-accent hover:underline"
        >
          Work orders
        </Link>
        <button
          type="button"
          onClick={onBack}
          className="ml-auto text-fg-3 hover:text-fg"
        >
          All properties
        </button>
      </div>
    </div>
  );
}
