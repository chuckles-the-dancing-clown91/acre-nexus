"use client";

// LLCs: the holding entities and the properties each one owns.

import { useMemo } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { motion } from "motion/react";
import { ChevronRight, Landmark } from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { usd } from "@/lib/format";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Stat } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel } from "@/components/ui/panel";

const rise = (i: number) => ({
  initial: { opacity: 0, y: 10 },
  animate: { opacity: 1, y: 0 },
  transition: {
    duration: 0.45,
    delay: Math.min(i, 12) * 0.035,
    ease: [0.22, 1, 0.36, 1] as const,
  },
});

export default function LlcsPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const groups = useQuery({
    queryKey: ["llc-groups"],
    queryFn: api.llcGroups,
    enabled: scoped && can("property:read"),
  });
  const totals = useMemo(() => {
    const g = groups.data ?? [];
    return {
      count: g.length,
      properties: g.reduce((n, x) => n + x.property_count, 0),
      units: g.reduce((n, x) => n + x.units, 0),
      rent: g.reduce((n, x) => n + x.monthly_rent_cents, 0),
    };
  }, [groups.data]);

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Portfolio"
        title="LLCs"
        description="Your holding entities and the properties they own."
      />

      {groups.data && groups.data.length > 0 && (
        <section className="grid grid-cols-2 gap-3 xl:grid-cols-4">
          <Stat label="Entities" value={totals.count} />
          <Stat label="Properties" value={totals.properties} />
          <Stat label="Units" value={totals.units} />
          <Stat label="Rent roll" value={usd(totals.rent)} hint="per month" />
        </section>
      )}

      {groups.isLoading && (
        <div className="space-y-3">
          {Array.from({ length: 3 }, (_, i) => (
            <Skeleton key={i} className="h-40 rounded-2xl" />
          ))}
        </div>
      )}
      {groups.error && (
        <Panel className="border-bad/30 p-4 text-[13px] text-bad">
          Couldn&apos;t load LLCs: {groups.error.message}
        </Panel>
      )}
      {groups.data?.length === 0 && (
        <Panel>
          <EmptyState
            icon={<Landmark />}
            title="No LLCs yet"
            description="Entities that hold title to your properties show up here."
          />
        </Panel>
      )}

      <div className="space-y-4">
        {groups.data?.map((g, i) => (
          <motion.div key={g.id} {...rise(i)}>
            <Panel className="overflow-hidden">
              <div className="flex flex-wrap items-center gap-x-5 gap-y-2 border-b border-line px-5 py-4">
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[15px] font-semibold text-fg">
                    {g.name}
                  </div>
                  <div className="text-xs text-fg-3">
                    EIN {g.ein || "—"} · {g.state || "—"}
                  </div>
                </div>
                <span className="text-[13px] text-fg-2">
                  <span className="figure text-fg">{g.property_count}</span>{" "}
                  {g.property_count === 1 ? "property" : "properties"}
                </span>
                <span className="text-[13px] text-fg-2">
                  <span className="figure text-fg">{g.units}</span> units
                </span>
                <span className="figure text-[13px] font-semibold text-fg">
                  {usd(g.monthly_rent_cents)}/mo
                </span>
                <Button size="sm" variant="secondary" asChild>
                  <Link href={`/console/llcs/${g.id}`}>
                    Manage entity
                    <ChevronRight />
                  </Link>
                </Button>
              </div>
              {g.properties.length === 0 ? (
                <p className="px-5 py-4 text-[13px] text-fg-3">
                  No properties held yet.
                </p>
              ) : (
                <ul className="divide-y divide-line">
                  {g.properties.map((p) => (
                    <li key={p.id}>
                      <Link
                        href={`/console/properties/${p.id}`}
                        className="flex items-center gap-4 px-5 py-3 transition hover:bg-fill-2"
                      >
                        <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg">
                          {p.name}
                        </span>
                        <span className="hidden text-xs text-fg-3 sm:block">
                          {p.occupancy}
                        </span>
                        <span className="figure text-[13px] text-fg-2">
                          {p.monthly_rent_label}
                        </span>
                        <Badge tone={statusTone(p.status)}>{p.status}</Badge>
                      </Link>
                    </li>
                  ))}
                </ul>
              )}
            </Panel>
          </motion.div>
        ))}
      </div>
    </div>
  );
}
