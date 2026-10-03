"use client";

// The equipment in a property: appliances and systems, each with its age,
// warranty, and a one-press work order to replace or service it.

import Link from "next/link";
import { useQuery } from "@tanstack/react-query";
import { Refrigerator, Wrench } from "lucide-react";
import { api } from "@/lib/api";
import { day, label, replacementKit } from "@/lib/propertyRecords";
import type { Asset } from "@/lib/types";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";

const ORDER = [
  "hvac",
  "plumbing",
  "appliance",
  "electrical",
  "safety",
  "structural",
  "other",
];

function lifeTone(a: Asset): "bad" | "warn" | "good" | "neutral" {
  if (a.years_left == null) return "neutral";
  if (a.years_left <= 0) return "bad";
  if (a.years_left <= 2) return "warn";
  return "good";
}

export function Systems({
  propertyId,
  canOrder,
}: {
  propertyId: string;
  canOrder: boolean;
}) {
  const assets = useQuery({
    queryKey: ["assets", propertyId],
    queryFn: () => api.assets({ property_id: propertyId }),
  });
  const rows = (assets.data ?? []).filter((a) => a.status === "active");
  const groups = ORDER.map(
    (k) => [k, rows.filter((a) => a.kind === k)] as const
  ).filter(([, v]) => v.length);

  return (
    <Panel>
      <PanelHeader
        title="Appliances and systems"
        description="Age, warranty and life left. Replace or service one straight from here."
        action={
          <Button size="sm" variant="ghost" asChild>
            <Link href="/console/maintenance">Equipment registry</Link>
          </Button>
        }
      />
      <div className="space-y-5 p-5 pt-4">
        {assets.isLoading && <Skeleton className="h-28" />}
        {assets.isSuccess && rows.length === 0 && (
          <EmptyState
            icon={<Refrigerator />}
            title="No equipment registered"
            description="Add the water heater, furnace and appliances so warranties and replacements are tracked."
            className="py-6"
          />
        )}
        {groups.map(([kind, list]) => (
          <section key={kind}>
            <div className="eyebrow mb-2">{label(kind)}</div>
            <ul className="divide-y divide-line rounded-xl border border-line">
              {list.map((a) => {
                const kit = replacementKit(a.name);
                const order = `/console/maintenance/new?property=${propertyId}${kit ? `&kit=${kit}` : ""}&note=${encodeURIComponent(
                  [a.name, a.make, a.model, a.location]
                    .filter(Boolean)
                    .join(" · ")
                )}`;
                return (
                  <li
                    key={a.id}
                    className="flex flex-col gap-2 px-3 py-2.5 sm:flex-row sm:items-center"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="text-[13px] font-medium text-fg">
                        {a.name}
                        {a.location && (
                          <span className="font-normal text-fg-3">
                            {" "}
                            · {a.location}
                          </span>
                        )}
                      </div>
                      <div className="text-xs text-fg-3">
                        {[a.make, a.model].filter(Boolean).join(" ")}
                        {a.install_date &&
                          ` · installed ${a.install_date.slice(0, 4)}`}
                        {a.serial_number && ` · SN ${a.serial_number}`}
                      </div>
                    </div>
                    <div className="flex shrink-0 flex-wrap items-center gap-1.5">
                      {a.years_left != null && (
                        <Badge tone={lifeTone(a)}>
                          {a.years_left <= 0
                            ? "past its life"
                            : `${a.years_left} yr left`}
                        </Badge>
                      )}
                      {a.warranty_state === "active" && a.warranty_expires && (
                        <Badge tone="info">
                          warranty to {day(a.warranty_expires)}
                        </Badge>
                      )}
                      {a.warranty_state === "expired" && (
                        <Badge>out of warranty</Badge>
                      )}
                      {canOrder && (
                        <Button size="sm" variant="ghost" asChild>
                          <Link href={order}>
                            <Wrench />
                            {kit?.startsWith("service")
                              ? "Service"
                              : kit
                                ? "Replace"
                                : "Work order"}
                          </Link>
                        </Button>
                      )}
                    </div>
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>
    </Panel>
  );
}
