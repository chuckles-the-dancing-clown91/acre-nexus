"use client";

// One unit: who lives there, its own appliances and meters, its utility
// agreement, and the work orders raised against it.

import Link from "next/link";
import { Suspense } from "react";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { useQuery } from "@tanstack/react-query";
import { ArrowLeft } from "lucide-react";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth";
import { meters, type UtilityTerm } from "@/lib/meters";
import { kindOf } from "@/lib/propertyKind";
import { Badge, statusTone } from "@/components/ui/badge";
import { Tabs } from "@/components/ui/data-table";
import { EmptyState, PageHeader, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { Meters } from "@/components/property/Meters";
import { Systems } from "@/components/property/Systems";
import { Fact } from "@/components/property/bits";

const TABS = [
  ["equipment", "Appliances"],
  ["meters", "Meters"],
  ["utilities", "Utility agreement"],
  ["work", "Work orders"],
] as const;
type Tab = (typeof TABS)[number][0];

export default function UnitPage() {
  return (
    <Suspense fallback={<Skeleton className="h-64 rounded-2xl" />}>
      <Unit />
    </Suspense>
  );
}

function Unit() {
  const { id, unitId } = useParams<{ id: string; unitId: string }>();
  const { can } = useAuth();
  const params = useSearchParams();
  const router = useRouter();
  const tab = (TABS.find(([k]) => k === params.get("tab"))?.[0] ??
    "equipment") as Tab;
  const property = useQuery({
    queryKey: ["property", id],
    queryFn: () => api.property(id),
  });
  const units = useQuery({
    queryKey: ["properties", id, "units"],
    queryFn: () => api.units(id),
  });
  const u = units.data?.find((x) => x.id === unitId);
  const kind = kindOf(property.data);
  const write = can("property:write") || can("maintenance:manage");

  return (
    <div className="space-y-6">
      <Link
        href={`/console/properties/${id}?tab=units`}
        className="inline-flex items-center gap-1.5 text-[13px] text-fg-3 hover:text-fg"
      >
        <ArrowLeft className="size-4" />
        {property.data?.name ?? "Property"}
      </Link>
      {units.isLoading ? (
        <Skeleton className="h-20 rounded-2xl" />
      ) : u ? (
        <PageHeader
          eyebrow={kind.unit}
          title={`${kind.unit} ${u.unit_number}`}
          description={
            <span className="flex flex-wrap items-center gap-2">
              <Badge tone={statusTone(u.status)}>
                {u.status.replace("_", " ")}
              </Badge>
              {[
                u.beds != null && `${u.beds} bd`,
                u.baths != null && `${u.baths} ba`,
                u.sqft != null && `${u.sqft} sqft`,
                u.floor != null && `floor ${u.floor}`,
              ]
                .filter(Boolean)
                .join(" · ")}
              {u.tenant_name && u.lease_id && (
                <Link
                  href={`/console/leases/${u.lease_id}`}
                  className="text-accent hover:underline"
                >
                  {u.tenant_name}
                </Link>
              )}
            </span>
          }
        />
      ) : (
        <EmptyState
          title="Unit not found"
          description="It may have been removed, or isn't in your view."
        />
      )}
      <Tabs
        tabs={TABS}
        value={tab}
        onChange={(k) => router.replace(`?tab=${k}`, { scroll: false })}
      />
      {u && tab === "equipment" && (
        <Systems
          propertyId={id}
          unitId={unitId}
          manage={write}
          canOrder={can("maintenance:manage")}
          title={`Appliances in ${u.unit_number}`}
          description="What this unit has, with age and warranty. Anything that serves the whole building is on the property."
        />
      )}
      {u && tab === "meters" && (
        <Meters
          propertyId={id}
          unitId={unitId}
          manage={write}
          title={`Meters for ${u.unit_number}`}
          description="Who pays for each utility in this unit, and the last reading."
        />
      )}
      {u && tab === "utilities" && (
        <UtilityAgreement propertyId={id} unitId={unitId} />
      )}
      {u && tab === "work" && <WorkOrders propertyId={id} unitId={unitId} />}
    </div>
  );
}

function UtilityAgreement({
  propertyId,
  unitId,
}: {
  propertyId: string;
  unitId: string;
}) {
  const q = useQuery({
    queryKey: ["utilities", propertyId, unitId],
    queryFn: () => meters.utilities(propertyId, unitId),
  });
  const rows: UtilityTerm[] = q.data ?? [];
  return (
    <Panel>
      <PanelHeader
        title="Utility agreement"
        description="What the lease will say about utilities for this unit, worked out from its meters and the building's."
      />
      <div className="p-5 pt-2">
        {q.isLoading && <Skeleton className="h-24" />}
        {q.isSuccess && rows.length === 0 && (
          <p className="text-[13px] text-fg-3">
            No meters yet, so the lease has nothing to say. Add meters first.
          </p>
        )}
        <dl className="divide-y divide-line">
          {rows.map((t) => (
            <div key={t.kind} className="py-2.5">
              <Fact label={t.label}>
                <span className="font-medium">{t.paid_by_label}</span>
                {t.provider && (
                  <span className="text-fg-3"> · {t.provider}</span>
                )}
              </Fact>
              {t.note && <p className="text-xs text-fg-3">{t.note}</p>}
            </div>
          ))}
        </dl>
      </div>
    </Panel>
  );
}

function WorkOrders({
  propertyId,
  unitId,
}: {
  propertyId: string;
  unitId: string;
}) {
  const q = useQuery({
    queryKey: ["properties", propertyId, "tickets"],
    queryFn: () => api.propertyTickets(propertyId),
  });
  const rows = (q.data ?? []).filter((t) => t.unit_id === unitId);
  return (
    <Panel>
      <PanelHeader
        title="Work orders"
        description="Everything raised against this unit."
      />
      <div className="p-2 pt-3">
        {q.isLoading && <Skeleton className="m-3 h-16" />}
        {q.isSuccess && rows.length === 0 && (
          <EmptyState title="None yet" className="py-8" />
        )}
        <ul className="divide-y divide-line">
          {rows.map((t) => (
            <li key={t.id}>
              <Link
                href={`/console/maintenance/${t.id}`}
                className="flex items-center gap-3 px-3 py-2.5 transition hover:bg-fill"
              >
                <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg">
                  {t.title}
                </span>
                <Badge tone={t.priority === "urgent" ? "bad" : "neutral"}>
                  {t.priority}
                </Badge>
                <Badge tone={statusTone(t.status)}>
                  {t.status.replace("_", " ")}
                </Badge>
              </Link>
            </li>
          ))}
        </ul>
      </div>
    </Panel>
  );
}
