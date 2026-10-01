"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { AnimatePresence, motion } from "motion/react";
import { ArrowUpRight } from "lucide-react";
import type { Property } from "@/lib/types";
import { cn } from "@/lib/utils";
import { IsoScene } from "@/components/iso/IsoScene";
import { occupancyTone, skylineBlocks } from "@/components/iso/skyline";
import type { IsoTone } from "@/components/iso/geometry";
import { Panel, PanelHeader } from "@/components/ui/panel";

const BAR: Record<IsoTone, string> = {
  accent: "bg-accent",
  good: "bg-good",
  warn: "bg-warn",
  bad: "bg-bad",
  info: "bg-info",
  plasma: "bg-plasma",
};
const TEXT: Record<IsoTone, string> = {
  accent: "text-accent",
  good: "text-good",
  warn: "text-warn",
  bad: "text-bad",
  info: "text-info",
  plasma: "text-plasma",
};

export function PortfolioSkyline({ properties }: { properties: Property[] }) {
  const router = useRouter();
  const [activeId, setActiveId] = useState<string | null>(null);
  const blocks = useMemo(() => skylineBlocks(properties), [properties]);
  const byId = useMemo(
    () => new Map(properties.map((p) => [p.id, p])),
    [properties]
  );
  const ordered = useMemo(
    () => blocks.map((b) => byId.get(b.id)!),
    [blocks, byId]
  );
  const active = activeId ? byId.get(activeId) : null;

  return (
    <Panel className="overflow-hidden">
      <PanelHeader
        title="Portfolio"
        description="Height shows units. Lit windows show occupancy."
        action={
          <Link
            href="/console/properties"
            className="flex shrink-0 items-center gap-1 text-[13px] font-medium whitespace-nowrap text-fg-3 transition hover:text-fg"
          >
            All properties
            <ArrowUpRight className="size-3.5" />
          </Link>
        }
      />
      <div className="grid lg:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)]">
        <div className="relative flex min-h-[320px] items-center justify-center px-6 pt-6 pb-12">
          <div className="w-full max-w-[560px]">
            <IsoScene
              blocks={blocks}
              scale={22}
              className="max-h-[360px]"
              activeId={activeId}
              onHover={setActiveId}
              onSelect={(id) => router.push(`/console/properties/${id}`)}
              labelFor={(b) => {
                const p = byId.get(b.id)!;
                return `${p.name}, ${p.occupied_units} of ${p.units} units occupied`;
              }}
            />
          </div>
          <AnimatePresence>
            {active && (
              <motion.div
                key={active.id}
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 6 }}
                transition={{ duration: 0.16 }}
                className="glass-strong pointer-events-none absolute bottom-11 left-6 w-60 rounded-xl p-3.5"
              >
                <div className="truncate text-[13px] font-semibold text-fg">
                  {active.name}
                </div>
                <div className="truncate text-xs text-fg-3">
                  {active.address} · {active.city}
                </div>
                <div className="mt-3 grid grid-cols-2 gap-2">
                  <div>
                    <div className="eyebrow">Occupied</div>
                    <div
                      className={cn(
                        "figure text-[15px] font-semibold",
                        TEXT[occupancyTone(active.occupied_units, active.units)]
                      )}
                    >
                      {active.occupied_units}/{active.units}
                    </div>
                  </div>
                  <div>
                    <div className="eyebrow">Rent / mo</div>
                    <div className="figure text-[15px] font-semibold text-fg">
                      {active.monthly_rent_label}
                    </div>
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>
          <div className="absolute bottom-4 left-6 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-fg-3">
            <Legend tone="accent" label="95%+ occupied" />
            <Legend tone="warn" label="80–95%" />
            <Legend tone="bad" label="Under 80%" />
          </div>
        </div>

        <ul className="divide-y divide-line border-t border-line lg:border-t-0 lg:border-l">
          {ordered.map((p) => {
            const tone = occupancyTone(p.occupied_units, p.units);
            const pct = p.units > 0 ? (p.occupied_units / p.units) * 100 : 0;
            return (
              <li key={p.id}>
                <Link
                  href={`/console/properties/${p.id}`}
                  onMouseEnter={() => setActiveId(p.id)}
                  onMouseLeave={() => setActiveId(null)}
                  onFocus={() => setActiveId(p.id)}
                  onBlur={() => setActiveId(null)}
                  className={cn(
                    "flex items-center gap-4 px-5 py-3.5 transition-colors",
                    activeId === p.id ? "bg-fill-2" : "hover:bg-fill"
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] font-medium text-fg">
                      {p.name}
                    </div>
                    <div className="truncate text-xs text-fg-3">{p.city}</div>
                  </div>
                  <div className="w-24 shrink-0">
                    <div className="flex items-baseline justify-between text-[11px]">
                      <span className="text-fg-3">Occ.</span>
                      <span
                        className={cn("font-mono tabular-nums", TEXT[tone])}
                      >
                        {p.occupied_units}/{p.units}
                      </span>
                    </div>
                    <div className="mt-1 h-1 overflow-hidden rounded-full bg-fill-2">
                      <div
                        className={cn("h-full rounded-full", BAR[tone])}
                        style={{ width: `${pct}%` }}
                      />
                    </div>
                  </div>
                  <div className="hidden w-24 shrink-0 text-right font-mono text-[12px] text-fg-2 tabular-nums sm:block">
                    {p.monthly_rent_label}
                  </div>
                </Link>
              </li>
            );
          })}
        </ul>
      </div>
    </Panel>
  );
}

function Legend({ tone, label }: { tone: IsoTone; label: string }) {
  return (
    <span className="flex items-center gap-1.5">
      <span className={cn("size-2 rounded-sm", BAR[tone])} />
      {label}
    </span>
  );
}
