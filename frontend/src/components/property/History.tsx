"use client";

// Everything that's happened to a property, newest first, with the estimate
// of its value and of its rent over time. Filter to sales and value, rent,
// loans and title, or permits.

import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Building2,
  FileBadge,
  Gavel,
  Home,
  KeyRound,
  Landmark,
  LineChart,
  Receipt,
  ShieldCheck,
  Tag,
  type LucideIcon,
} from "lucide-react";
import { api } from "@/lib/api";
import { usd } from "@/lib/format";
import { day, records, type TimelineEvent } from "@/lib/propertyRecords";
import { TrendChart } from "@/components/charts";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const ICON: Record<string, LucideIcon> = {
  built: Building2,
  sale: Tag,
  estimate: LineChart,
  tax: Receipt,
  lease: KeyRound,
  listing: Home,
  loan: Landmark,
  deed: FileBadge,
  lien: Gavel,
  permit: FileBadge,
  insurance: ShieldCheck,
};

const FILTERS = [
  { key: "all", label: "All", kinds: null },
  {
    key: "value",
    label: "Sales and value",
    kinds: ["built", "sale", "estimate", "tax"],
  },
  { key: "rent", label: "Rent", kinds: ["lease", "listing"] },
  { key: "title", label: "Loans and title", kinds: ["loan", "deed", "lien"] },
  { key: "work", label: "Permits and cover", kinds: ["permit", "insurance"] },
] as const;

/** The estimate over time, oldest first, one point per date. */
export function series(
  v: { as_of: string; value: number | null | undefined }[]
): { months: string[]; values: number[] } {
  const byDate = new Map<string, number>();
  for (const x of v) if (x.value != null) byDate.set(x.as_of, x.value);
  const dates = [...byDate.keys()].sort();
  return {
    months: dates.map((d) => d.slice(0, 7)),
    values: dates.map((d) => byDate.get(d) as number),
  };
}

export function History({ propertyId }: { propertyId: string }) {
  const timeline = useQuery({
    queryKey: ["timeline", propertyId],
    queryFn: () => records.timeline(propertyId),
  });
  const intel = useQuery({
    queryKey: ["intel", propertyId],
    queryFn: () => api.propertyIntel(propertyId),
  });
  const [filter, setFilter] = useState<(typeof FILTERS)[number]["key"]>("all");

  const valuations = useMemo(() => intel.data?.valuations ?? [], [intel.data]);
  const worth = useMemo(
    () =>
      series(
        valuations.map((x) => ({
          as_of: x.as_of,
          value: x.estimated_value_cents,
        }))
      ),
    [valuations]
  );
  const rent = useMemo(
    () =>
      series(
        valuations.map((x) => ({
          as_of: x.as_of,
          value: x.estimated_rent_cents,
        }))
      ),
    [valuations]
  );
  const kinds = FILTERS.find((f) => f.key === filter)?.kinds ?? null;
  const rows = (timeline.data ?? []).filter(
    (e) => !kinds || (kinds as readonly string[]).includes(e.kind)
  );

  return (
    <div className="space-y-4">
      {worth.values.length >= 2 && (
        <div className="grid gap-4 md:grid-cols-2">
          <TrendChart
            title="Estimated value"
            months={worth.months}
            values={worth.values.map((c) => c / 100)}
            format={(v) => usd(Math.round(v) * 100)}
          />
          {rent.values.length >= 2 && (
            <TrendChart
              title="Rent estimate, per month"
              months={rent.months}
              values={rent.values.map((c) => c / 100)}
              format={(v) => usd(Math.round(v) * 100)}
              tone="info"
            />
          )}
        </div>
      )}
      {worth.values.length === 1 && (
        <p className="text-[13px] text-fg-3">
          One value estimate so far. Refresh the public records on the Parcel
          tab over time to see how it moves.
        </p>
      )}

      <Panel>
        <PanelHeader
          title="History"
          description="Sales, estimates, taxes, leases, loans, permits and policies."
        />
        <div className="space-y-3 p-5 pt-4">
          <div className="flex flex-wrap gap-1.5">
            {FILTERS.map((f) => (
              <button
                key={f.key}
                type="button"
                onClick={() => setFilter(f.key)}
                className={cn(
                  "rounded-full border px-2.5 py-1 text-xs transition",
                  filter === f.key
                    ? "border-accent bg-accent/10 text-accent"
                    : "border-line text-fg-3 hover:text-fg"
                )}
              >
                {f.label}
              </button>
            ))}
          </div>
          {timeline.isLoading && <Skeleton className="h-32" />}
          {timeline.isSuccess && rows.length === 0 && (
            <EmptyState
              icon={<LineChart />}
              title="Nothing here yet"
              className="py-6"
            />
          )}
          <ol className="divide-y divide-line">
            {rows.map((e, i) => (
              <Entry key={`${e.date}-${e.kind}-${i}`} e={e} />
            ))}
          </ol>
        </div>
      </Panel>
    </div>
  );
}

function Entry({ e }: { e: TimelineEvent }) {
  const Icon = ICON[e.kind] ?? Tag;
  return (
    <li className="flex items-start gap-3 py-2.5">
      <span className="mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg border border-line bg-fill text-fg-2">
        <Icon className="size-4" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium text-fg">
          {e.title.charAt(0).toUpperCase() + e.title.slice(1)}
        </div>
        {e.detail && <div className="text-xs text-fg-3">{e.detail}</div>}
      </div>
      <div className="shrink-0 text-right">
        {e.amount_label && (
          <div className="figure text-[13px] text-fg">{e.amount_label}</div>
        )}
        <div className="text-xs text-fg-3">{day(e.date)}</div>
      </div>
    </li>
  );
}
