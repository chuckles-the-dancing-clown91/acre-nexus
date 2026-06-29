"use client";

import { useRouter } from "next/navigation";
import Link from "next/link";
import { Building2, Plus } from "lucide-react";

import { useAuth } from "@/lib/auth";
import { useProperties } from "@/lib/queries";
import { titleCase } from "@/lib/format";
import { Badge, statusTone } from "@/components/ui";
import { Button } from "@/components/ui/button";
import { PageHeader, EmptyState } from "@/components/ui/page";
import { DataTable, type ColumnDef } from "@/components/ui/data-table";
import type { Property } from "@/lib/types";

/** Thin occupancy progress bar + "11/12" text, mirroring the dashboard. */
function OccupancyBar({ property }: { property: Property }) {
  const pct = property.units
    ? Math.round((property.occupied_units / property.units) * 100)
    : 0;
  return (
    <div className="flex items-center gap-2">
      <div className="h-1.5 w-20 overflow-hidden rounded-full bg-surface-2">
        <div
          className="h-full rounded-full bg-accent"
          style={{ width: `${pct}%` }}
        />
      </div>
      <span data-numeric className="text-xs text-ink-2">
        {property.occupancy}
      </span>
    </div>
  );
}

const columns: ColumnDef<Property>[] = [
  {
    accessorKey: "name",
    header: "Property",
    cell: ({ row }) => {
      const p = row.original;
      return (
        <div className="min-w-0">
          <div className="truncate font-medium text-ink">{p.name}</div>
          <div className="truncate text-xs text-ink-3">
            {p.address}
            {p.city ? ` · ${p.city}` : ""}
          </div>
        </div>
      );
    },
  },
  {
    id: "occupancy",
    accessorKey: "occupied_units",
    header: "Occupancy",
    cell: ({ row }) => <OccupancyBar property={row.original} />,
  },
  {
    accessorKey: "monthly_rent_cents",
    header: () => <div className="text-right">Monthly rent</div>,
    cell: ({ row }) => (
      <div
        data-numeric
        className="text-right font-medium text-ink"
      >
        {row.original.monthly_rent_label}
      </div>
    ),
  },
  {
    accessorKey: "status",
    header: "Status",
    cell: ({ row }) => (
      <Badge tone={statusTone(row.original.status)}>
        {row.original.status}
      </Badge>
    ),
  },
  {
    accessorKey: "property_type",
    header: "Type",
    cell: ({ row }) => (
      <span className="text-ink-2">{titleCase(row.original.property_type)}</span>
    ),
  },
];

export default function PropertiesPage() {
  const { can } = useAuth();
  const router = useRouter();
  const props = useProperties();

  return (
    <div className="space-y-6">
      <PageHeader
        eyebrow="Portfolio"
        title="Properties"
        description="Every asset in your managed portfolio."
        actions={
          can("property:write") ? (
            <Button asChild>
              <Link href="/console/properties/onboard">
                <Plus className="h-4 w-4" />
                Onboard property
              </Link>
            </Button>
          ) : undefined
        }
      />

      <DataTable<Property>
        columns={columns}
        data={props.data ?? []}
        isLoading={props.isLoading}
        searchPlaceholder="Search properties…"
        onRowClick={(p) => router.push(`/console/properties/${p.id}`)}
        emptyState={
          <EmptyState
            className="border-0"
            icon={Building2}
            title="No properties yet"
            description="Onboard your first property to start tracking occupancy, rent, and maintenance."
            action={
              can("property:write") ? (
                <Button asChild>
                  <Link href="/console/properties/onboard">
                    Onboard property
                  </Link>
                </Button>
              ) : undefined
            }
          />
        }
      />
    </div>
  );
}
