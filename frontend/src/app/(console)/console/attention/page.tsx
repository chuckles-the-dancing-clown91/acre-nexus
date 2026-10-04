"use client";

// Needs attention, across the portfolio: owner approvals waiting, work orders
// with no date, routines due, property records to act on, and vendor
// paperwork. One list, grouped, each line going to where it gets done.

import { useMemo, useState } from "react";
import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import {
  BadgeDollarSign,
  CalendarClock,
  ChevronRight,
  ClipboardList,
  FileWarning,
  HardHat,
  TriangleAlert,
} from "lucide-react";
import {
  attention,
  groupByKind,
  KIND_WORDS,
  type AttentionItem,
  type AttentionKind,
} from "@/lib/attention";
import { useAuth } from "@/lib/auth";
import { useHasTenantScope } from "@/components/shell/tenant-scope";
import { Badge, type Tone } from "@/components/ui/badge";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { cn } from "@/lib/utils";

const ICON: Record<AttentionKind, React.ReactNode> = {
  approval: <BadgeDollarSign />,
  unscheduled: <ClipboardList />,
  routine: <CalendarClock />,
  profile: <FileWarning />,
  vendor: <HardHat />,
};

const HINT: Record<AttentionKind, string> = {
  approval:
    "The owner was asked and hasn't answered. Nudge them or go ahead with a reason.",
  unscheduled:
    "Open with nobody booked. Offer the resident times or set a date.",
  routine: "Coming due within the lead time. Open the work order and book it.",
  profile: "From what's on file: permits, insurance, appliances, safety.",
  vendor: "W-9s and insurance certificates to chase before the next job.",
};

function tone(p: AttentionItem["priority"]): Tone {
  return p === "high" ? "bad" : p === "low" ? "neutral" : "warn";
}

export default function AttentionPage() {
  const { can } = useAuth();
  const scoped = useHasTenantScope();
  const q = useQuery({
    queryKey: ["attention", "portfolio"],
    queryFn: attention.portfolio,
    enabled: scoped && can("property:read"),
  });
  const [only, setOnly] = useState<AttentionKind | "all">("all");
  const items = useMemo(() => q.data?.items ?? [], [q.data]);
  const groups = useMemo(
    () => groupByKind(items).filter(([k]) => only === "all" || k === only),
    [items, only]
  );
  const high = items.filter((i) => i.priority === "high").length;

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Portfolio"
        title="Needs attention"
        description="Everything waiting on someone, across every property you see."
      />

      {q.isLoading && <Skeleton className="h-64 rounded-2xl" />}

      {q.data && (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => setOnly("all")}
              className={cn(
                "rounded-full border px-3 py-1 text-[13px] transition",
                only === "all"
                  ? "border-accent bg-accent/10 text-accent"
                  : "border-line text-fg-3 hover:text-fg"
              )}
            >
              All · {items.length}
            </button>
            {(Object.keys(KIND_WORDS) as AttentionKind[])
              .filter((k) => (q.data.counts[k] ?? 0) > 0)
              .map((k) => (
                <button
                  key={k}
                  type="button"
                  onClick={() => setOnly(k)}
                  className={cn(
                    "rounded-full border px-3 py-1 text-[13px] transition",
                    only === k
                      ? "border-accent bg-accent/10 text-accent"
                      : "border-line text-fg-3 hover:text-fg"
                  )}
                >
                  {KIND_WORDS[k]} · {q.data.counts[k]}
                </button>
              ))}
            {high > 0 && (
              <span className="ml-auto flex items-center gap-1.5 text-[13px] text-bad">
                <TriangleAlert className="size-4" />
                {high} high
              </span>
            )}
          </div>

          {items.length === 0 && (
            <Panel>
              <EmptyState
                icon={<TriangleAlert />}
                title="Nothing needs you"
                description="Approvals answered, work orders booked, routines on schedule, records current."
              />
            </Panel>
          )}

          {groups.map(([kind, rows]) => (
            <Panel key={kind}>
              <PanelHeader
                title={KIND_WORDS[kind]}
                description={HINT[kind]}
                action={<Badge tone="neutral">{rows.length}</Badge>}
              />
              <ul className="divide-y divide-line">
                {rows.map((i) => (
                  <li key={`${i.kind}:${i.key}`}>
                    <Link
                      href={i.href}
                      className="flex items-start gap-3 px-5 py-3 transition hover:bg-fill"
                    >
                      <span
                        className={cn(
                          "mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg border [&_svg]:size-4",
                          i.priority === "high"
                            ? "border-bad/30 bg-bad/10 text-bad"
                            : "border-line bg-fill text-fg-2"
                        )}
                      >
                        {ICON[kind]}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="flex flex-wrap items-center gap-2">
                          <span className="text-[14px] font-medium text-fg">
                            {i.title}
                          </span>
                          <Badge tone={tone(i.priority)}>{i.priority}</Badge>
                          {i.due_on && (
                            <span className="figure text-xs text-fg-3">
                              {i.due_on}
                            </span>
                          )}
                        </span>
                        <span className="mt-0.5 block text-[13px] text-fg-2">
                          {i.detail}
                        </span>
                        {i.property_name && (
                          <span className="mt-0.5 block text-xs text-fg-3">
                            {i.property_name}
                          </span>
                        )}
                      </span>
                      <ChevronRight className="mt-2 size-4 shrink-0 text-fg-4" />
                    </Link>
                  </li>
                ))}
              </ul>
            </Panel>
          ))}
        </>
      )}
    </div>
  );
}
