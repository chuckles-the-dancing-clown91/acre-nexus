"use client";

// Crime around the property: the nearest reporting agency's yearly rates
// per 100,000 against the state and the country, from the FBI's Crime Data
// Explorer when the workspace has it on. One line says how the area reads.

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { ShieldAlert, ShieldCheck } from "lucide-react";
import { api } from "@/lib/api";
import type { CrimeOffense, CrimeStats } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

/** "01-2025" → "Jan 2025". */
export function monthWords(mmyyyy: string): string {
  const [m, y] = mmyyyy.split("-");
  const i = Number(m) - 1;
  const names = [
    "Jan",
    "Feb",
    "Mar",
    "Apr",
    "May",
    "Jun",
    "Jul",
    "Aug",
    "Sep",
    "Oct",
    "Nov",
    "Dec",
  ];
  return i >= 0 && i < 12 ? `${names[i]} ${y}` : mmyyyy;
}

/** How an offense compares with the state: a ratio and words. */
export function compare(o: CrimeOffense): {
  ratio: number | null;
  words: string;
} {
  if (!o.state_rate) return { ratio: null, words: "no state figure" };
  const ratio = o.agency_rate / o.state_rate;
  const pct = Math.round(Math.abs(ratio - 1) * 100);
  if (pct < 10) return { ratio, words: "about the state rate" };
  return {
    ratio,
    words: ratio > 1 ? `${pct}% above the state` : `${pct}% below the state`,
  };
}

/** Year-over-year change in the agency's rate, in words. */
export function trendWords(o: CrimeOffense): string | null {
  if (o.prior_rate == null || o.prior_rate <= 0) return null;
  const pct = Math.round(((o.agency_rate - o.prior_rate) / o.prior_rate) * 100);
  if (Math.abs(pct) < 3) return "flat on the year before";
  return pct > 0
    ? `up ${pct}% on the year before`
    : `down ${-pct}% on the year before`;
}

function tone(verdict: string): "good" | "warn" | "bad" | "neutral" {
  switch (verdict) {
    case "well_below":
    case "below":
      return "good";
    case "above":
      return "warn";
    case "well_above":
      return "bad";
    default:
      return "neutral";
  }
}

export function Safety({ propertyId }: { propertyId: string }) {
  const intel = useQuery({
    queryKey: ["intel", propertyId],
    queryFn: () => api.propertyIntel(propertyId),
  });
  const c: CrimeStats | null | undefined = intel.data?.crime;
  if (!c) return null;
  const t = tone(c.verdict);
  const max = Math.max(
    1,
    ...c.offenses.flatMap((o) => [o.agency_rate, o.state_rate, o.us_rate])
  );
  return (
    <Panel className="mb-4">
      <PanelHeader
        title={
          <span className="flex items-center gap-2">
            {t === "bad" || t === "warn" ? (
              <ShieldAlert
                className={cn("size-4", t === "bad" ? "text-bad" : "text-warn")}
              />
            ) : (
              <ShieldCheck className="size-4 text-good" />
            )}
            Safety
            <Badge tone={t}>{c.verdict_words}</Badge>
            {c.simulated && <Badge tone="neutral">Sample figures</Badge>}
          </span>
        }
        description={`${c.agency_name}${c.agency_km != null ? `, ${c.agency_km} km away` : ""}, ${monthWords(c.period_from)} to ${monthWords(c.period_to)}${c.population ? `, serving ${c.population.toLocaleString()} people` : ""}.`}
      />
      <div className="divide-y divide-line px-5 pb-4">
        {c.offenses.map((o) => {
          const cmp = compare(o);
          const trend = trendWords(o);
          return (
            <div key={o.key} className="py-3">
              <div className="flex items-baseline justify-between gap-3">
                <div className="text-[13px] font-medium text-fg">{o.label}</div>
                <div className="text-right text-[12px] text-fg-3">
                  <span
                    className={cn(
                      "figure text-[15px] font-semibold",
                      cmp.ratio != null && cmp.ratio > 1.15
                        ? "text-bad"
                        : cmp.ratio != null && cmp.ratio < 0.85
                          ? "text-good"
                          : "text-fg"
                    )}
                  >
                    {Math.round(o.agency_rate).toLocaleString()}
                  </span>{" "}
                  per 100k · {cmp.words}
                  {trend ? `, ${trend}` : ""}
                </div>
              </div>
              <div className="mt-1.5 space-y-1">
                {[
                  {
                    label: "Here",
                    v: o.agency_rate,
                    cls:
                      t === "bad"
                        ? "bg-bad"
                        : t === "warn"
                          ? "bg-warn"
                          : "bg-accent",
                  },
                  { label: "State", v: o.state_rate, cls: "bg-fg-4" },
                  { label: "U.S.", v: o.us_rate, cls: "bg-fg-4/60" },
                ].map((bar) => (
                  <div
                    key={bar.label}
                    className="flex items-center gap-2 text-[11px] text-fg-3"
                  >
                    <span className="w-9 shrink-0">{bar.label}</span>
                    <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-fill">
                      <div
                        className={cn("h-full rounded-full", bar.cls)}
                        style={{
                          width: `${Math.min(100, (bar.v / max) * 100)}%`,
                        }}
                      />
                    </div>
                    <span className="figure w-14 shrink-0 text-right">
                      {Math.round(bar.v).toLocaleString()}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          );
        })}
      </div>
      <div className="border-t border-line px-5 py-2.5 text-[11px] text-fg-4">
        {c.simulated ? (
          <>
            Stand-in figures. Turn on the FBI feed under{" "}
            <Link href="/console/settings" className="text-accent">
              Settings, Property data
            </Link>{" "}
            for the real ones.
          </>
        ) : (
          <>
            FBI Crime Data Explorer, offenses per 100,000 people a year, summed
            from monthly reports. Agencies report on different schedules; recent
            months can be incomplete.
          </>
        )}
      </div>
    </Panel>
  );
}
