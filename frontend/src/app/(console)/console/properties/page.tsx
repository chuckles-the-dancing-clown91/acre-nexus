"use client";

// Properties: the portfolio as the viewer sees it. The company sees every
// property; a property manager, leasing agent, maintenance tech or owner sees
// only the ones assigned to them (the server narrows the list).

import { useMemo, useState } from "react";
import Link from "next/link";
import { motion } from "motion/react";
import { Building2, MapPin, Plus, Search } from "lucide-react";
import { useAuth } from "@/lib/auth";
import { useProperties } from "@/lib/queries";
import { usd } from "@/lib/format";
import { useHasTenantScope, useReach } from "@/components/shell/tenant-scope";
import { Ring } from "@/components/charts";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";
import { matchesProperty, occupancyPct } from "@/lib/properties";
import type { Property } from "@/lib/types";

const rise = (i: number) => ({
  initial: { opacity: 0, y: 10 },
  animate: { opacity: 1, y: 0 },
  transition: {
    duration: 0.45,
    delay: Math.min(i, 12) * 0.035,
    ease: [0.22, 1, 0.36, 1] as const,
  },
});

export default function PropertiesPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const { scoped: propertyScoped } = useReach();
  const { data, error, isLoading } = useProperties({ enabled: scoped });
  const [q, setQ] = useState("");

  const rows = useMemo(
    () => (data ?? []).filter((p) => matchesProperty(p, q)),
    [data, q]
  );
  const totals = useMemo(() => {
    const all = data ?? [];
    const units = all.reduce((n, p) => n + p.units, 0);
    const occupied = all.reduce((n, p) => n + p.occupied_units, 0);
    return {
      count: all.length,
      units,
      occupancy: occupancyPct(units, occupied),
      rent: all.reduce((n, p) => n + p.monthly_rent_cents, 0),
    };
  }, [data]);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow={propertyScoped ? "Assigned to you" : "Portfolio"}
        title="Properties"
        description={
          data ? (
            <>
              {totals.count} {totals.count === 1 ? "property" : "properties"} ·{" "}
              {totals.units} units · {totals.occupancy}% occupied ·{" "}
              {usd(totals.rent)}/mo
            </>
          ) : (
            <Skeleton className="h-5 w-72" />
          )
        }
        actions={
          can("property:write") &&
          !propertyScoped && (
            <Button asChild>
              <Link href="/console/properties/onboard">
                <Plus />
                Add property
              </Link>
            </Button>
          )
        }
      />

      {(data?.length ?? 0) > 6 && (
        <div className="relative max-w-sm">
          <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3" />
          <Input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="Filter by name, address or manager"
            aria-label="Filter properties"
            className="pl-9"
          />
        </div>
      )}

      {error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load properties: {error.message}
        </Panel>
      )}

      {isLoading && (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 6 }, (_, i) => (
            <Skeleton key={i} className="h-[248px] rounded-2xl" />
          ))}
        </div>
      )}

      {data && data.length === 0 && (
        <Panel>
          <EmptyState
            icon={<Building2 />}
            title={
              propertyScoped
                ? "No properties assigned to you yet"
                : "No properties yet"
            }
            description={
              propertyScoped
                ? "When your company assigns you to a property, it shows up here."
                : "Add your first property to start tracking units, leases and work orders."
            }
            action={
              !propertyScoped &&
              can("property:write") && (
                <Button asChild>
                  <Link href="/console/properties/onboard">
                    <Plus />
                    Add property
                  </Link>
                </Button>
              )
            }
          />
        </Panel>
      )}

      {data && data.length > 0 && rows.length === 0 && (
        <Panel>
          <EmptyState
            icon={<Search />}
            title="Nothing matches"
            description={`No property matches “${q}”.`}
          />
        </Panel>
      )}

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
        {rows.map((p, i) => (
          <motion.div key={p.id} {...rise(i)}>
            <PropertyCard property={p} />
          </motion.div>
        ))}
      </div>
    </div>
  );
}

function PropertyCard({ property: p }: { property: Property }) {
  const occ = occupancyPct(p.units, p.occupied_units);
  return (
    <Link
      href={`/console/properties/${p.id}`}
      className="group block rounded-2xl outline-none focus-visible:ring-2 focus-visible:ring-accent"
    >
      <Panel interactive className="overflow-hidden">
        <div className="relative h-32 overflow-hidden rounded-t-2xl bg-fill">
          {p.image_url ? (
            // eslint-disable-next-line @next/next/no-img-element -- street photo from our blob store
            <img
              src={p.image_url}
              alt=""
              className="size-full object-cover opacity-90 transition duration-500 group-hover:scale-[1.03] group-hover:opacity-100"
            />
          ) : (
            <div className="size-full bg-[radial-gradient(120%_120%_at_0%_0%,color-mix(in_oklab,var(--accent)_28%,transparent),transparent)]" />
          )}
          <div className="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-black/50 to-transparent" />
          <div className="absolute top-3 left-3">
            <Badge tone={statusTone(p.status)}>{p.status}</Badge>
          </div>
        </div>
        <div className="flex items-start gap-4 p-4">
          <div className="min-w-0 flex-1">
            <div className="truncate text-[15px] font-semibold text-fg">
              {p.name}
            </div>
            <div className="mt-0.5 flex items-center gap-1 truncate text-[13px] text-fg-3">
              <MapPin className="size-3.5 shrink-0" />
              <span className="truncate">
                {p.address}, {p.city}
              </span>
            </div>
            <div className="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-xs text-fg-2">
              <span>
                <span className="figure text-fg">{p.units}</span> units
              </span>
              <span>
                <span className="figure text-fg">
                  {usd(p.monthly_rent_cents)}
                </span>
                /mo
              </span>
              {p.manager && <span className="truncate">{p.manager}</span>}
            </div>
          </div>
          <div className="flex flex-col items-center gap-1">
            <Ring
              value={occ}
              size={46}
              tone={occ >= 95 ? "good" : occ >= 85 ? "warn" : "bad"}
            />
            <span className="text-[10px] text-fg-3">occupied</span>
          </div>
        </div>
      </Panel>
    </Link>
  );
}
