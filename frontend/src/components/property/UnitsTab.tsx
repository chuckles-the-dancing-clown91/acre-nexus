"use client";

// A building's units: who's in each, what each has in it, and a way to add
// the next one. Each unit opens to its own appliances and meters.

import { useState } from "react";
import Link from "next/link";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { DoorOpen, Gauge, Plus, Refrigerator, Wrench } from "lucide-react";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { dollarsToCents } from "@/lib/showings";
import type { Kind } from "@/lib/propertyKind";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { EmptyState, Skeleton } from "@/components/ui/misc";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, FormDialog, input, why } from "./bits";

export function UnitsTab({
  propertyId,
  kind,
  manage,
  canSeeLeases,
}: {
  propertyId: string;
  kind: Kind;
  manage: boolean;
  canSeeLeases: boolean;
}) {
  const qc = useQueryClient();
  const q = useQuery({
    queryKey: ["properties", propertyId, "units"],
    queryFn: () => api.units(propertyId),
  });
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [d, setD] = useState({
    unit_number: "",
    floor: "",
    beds: "",
    baths: "",
    sqft: "",
    rent: "",
  });
  const unit = kind.unit.toLowerCase();

  async function add() {
    if (!d.unit_number.trim()) return void toast.error(`Name the ${unit}`);
    const rent = d.rent ? dollarsToCents(d.rent) : null;
    if (d.rent && rent == null)
      return void toast.error("Enter the rent as dollars");
    setBusy(true);
    try {
      await api.createUnit(propertyId, {
        unit_number: d.unit_number.trim(),
        ...(d.floor ? { floor: Number(d.floor) } : {}),
        ...(d.beds ? { beds: Number(d.beds) } : {}),
        ...(d.baths ? { baths: Number(d.baths) } : {}),
        ...(d.sqft ? { sqft: Number(d.sqft) } : {}),
        ...(rent != null ? { market_rent_cents: rent } : {}),
      });
      toast.success(`${kind.unit} added`);
      setOpen(false);
      setD({
        unit_number: "",
        floor: "",
        beds: "",
        baths: "",
        sqft: "",
        rent: "",
      });
      void qc.invalidateQueries({
        queryKey: ["properties", propertyId, "units"],
      });
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  const rows = q.data ?? [];
  const occupied = rows.filter((u) => u.status === "occupied").length;
  return (
    <Panel>
      <PanelHeader
        title={kind.units}
        description={
          q.data ? `${rows.length} on record · ${occupied} occupied` : undefined
        }
        action={
          manage && (
            <Button size="sm" variant="secondary" onClick={() => setOpen(true)}>
              <Plus />
              Add {unit}
            </Button>
          )
        }
      />
      <div className="p-2 pt-3">
        {q.isLoading && <Skeleton className="m-3 h-24" />}
        {q.isSuccess && rows.length === 0 && (
          <EmptyState
            icon={<DoorOpen />}
            title={`No ${kind.units.toLowerCase()} yet`}
            description={`Add each ${unit} so tenants, appliances and meters can sit where they belong.`}
            className="py-8"
          />
        )}
        {rows.length > 0 && (
          <div className="overflow-x-auto">
            <table className="w-full text-[13px]">
              <thead>
                <tr className="text-left text-[11px] tracking-wide text-fg-3 uppercase">
                  <th className="px-3 py-2 font-medium">{kind.unit}</th>
                  <th className="px-3 py-2 font-medium">Layout</th>
                  {canSeeLeases && (
                    <th className="px-3 py-2 font-medium">Tenant</th>
                  )}
                  <th className="px-3 py-2 font-medium">In it</th>
                  <th className="px-3 py-2 text-right font-medium">
                    Market rent
                  </th>
                  <th className="px-3 py-2 text-right font-medium">Status</th>
                </tr>
              </thead>
              <tbody>
                {rows.map((u) => (
                  <tr
                    key={u.id}
                    className="border-t border-line text-fg-2 transition hover:bg-fill"
                  >
                    <td className="px-3 py-2.5 font-medium text-fg">
                      <Link
                        href={`/console/properties/${propertyId}/units/${u.id}`}
                        className="hover:text-accent"
                      >
                        {u.unit_number}
                      </Link>
                      {u.floor != null && (
                        <span className="font-normal text-fg-3">
                          {" "}
                          · floor {u.floor}
                        </span>
                      )}
                    </td>
                    <td className="px-3 py-2.5">
                      {[
                        u.beds != null && `${u.beds} bd`,
                        u.baths != null && `${u.baths} ba`,
                        u.sqft != null && `${u.sqft} sqft`,
                      ]
                        .filter(Boolean)
                        .join(" · ") || "—"}
                    </td>
                    {canSeeLeases && (
                      <td className="px-3 py-2.5">
                        {u.lease_id ? (
                          <Link
                            href={`/console/leases/${u.lease_id}`}
                            className="text-accent hover:underline"
                          >
                            {u.tenant_name}
                          </Link>
                        ) : (
                          <span className="text-fg-3">None</span>
                        )}
                      </td>
                    )}
                    <td className="px-3 py-2.5">
                      <span className="flex items-center gap-3 text-fg-3">
                        <span
                          className="inline-flex items-center gap-1"
                          title="Appliances"
                        >
                          <Refrigerator className="size-3.5" />
                          {u.appliances ?? 0}
                        </span>
                        <span
                          className="inline-flex items-center gap-1"
                          title="Meters"
                        >
                          <Gauge className="size-3.5" />
                          {u.meters ?? 0}
                        </span>
                        {(u.open_tickets ?? 0) > 0 && (
                          <span
                            className="inline-flex items-center gap-1 text-warn"
                            title="Open work orders"
                          >
                            <Wrench className="size-3.5" />
                            {u.open_tickets}
                          </span>
                        )}
                      </span>
                    </td>
                    <td className="figure px-3 py-2.5 text-right">
                      {u.market_rent_label ?? "—"}
                    </td>
                    <td className="px-3 py-2.5 text-right">
                      <Badge tone={statusTone(u.status)}>
                        {u.status.replace("_", " ")}
                      </Badge>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
      <FormDialog
        open={open}
        onOpenChange={setOpen}
        title={`Add ${unit}`}
        busy={busy}
        onSave={() => void add()}
      >
        <div className="grid gap-3 sm:grid-cols-2">
          <F label={`${kind.unit} number`}>
            <input
              className={input}
              placeholder="2B"
              value={d.unit_number}
              onChange={(e) => setD({ ...d, unit_number: e.target.value })}
            />
          </F>
          <F label="Floor">
            <input
              type="number"
              className={input}
              value={d.floor}
              onChange={(e) => setD({ ...d, floor: e.target.value })}
            />
          </F>
          <F label="Bedrooms">
            <input
              type="number"
              min={0}
              className={input}
              value={d.beds}
              onChange={(e) => setD({ ...d, beds: e.target.value })}
            />
          </F>
          <F label="Bathrooms">
            <input
              type="number"
              min={0}
              step="0.5"
              className={input}
              value={d.baths}
              onChange={(e) => setD({ ...d, baths: e.target.value })}
            />
          </F>
          <F label="Square feet">
            <input
              type="number"
              min={0}
              className={input}
              value={d.sqft}
              onChange={(e) => setD({ ...d, sqft: e.target.value })}
            />
          </F>
          <F label="Market rent ($ a month)">
            <input
              className={input}
              inputMode="decimal"
              value={d.rent}
              onChange={(e) => setD({ ...d, rent: e.target.value })}
            />
          </F>
        </div>
      </FormDialog>
    </Panel>
  );
}
