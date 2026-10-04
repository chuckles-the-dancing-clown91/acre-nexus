"use client";

// What kind of property this is. It decides how the property is rented and
// which tabs it has: a house is one unit, a building has many, a campground
// has sites on its map.

import { useState } from "react";
import Link from "next/link";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/lib/api";
import { KINDS, kindOf, type Kind } from "@/lib/propertyKind";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, input, why } from "./bits";

const HOW: Record<Kind["mode"], string> = {
  single:
    "Rented as one home. Its lease, appliances and meters belong to the home.",
  multi:
    "A building of rentable spaces. Each has its own tenant, appliances and meters.",
  sites:
    "Rented by the night. Sites are drawn on the site map and booked from Campgrounds.",
  none: "Nothing to rent here yet.",
};

export function KindCard({
  property,
  propertyId,
  manage,
}: {
  property: { property_type: string | null; units: number } | undefined;
  propertyId: string;
  manage: boolean;
}) {
  const qc = useQueryClient();
  const kind = kindOf(property);
  const [pick, setPick] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const value = pick ?? kind.key;

  async function save() {
    setBusy(true);
    try {
      await api.updateProperty(propertyId, { property_type: value });
      toast.success("Property type saved");
      setPick(null);
      void qc.invalidateQueries({ queryKey: ["properties"] });
      void qc.invalidateQueries({ queryKey: ["property", propertyId] });
      void qc.invalidateQueries({ queryKey: ["intel", propertyId] });
      void qc.invalidateQueries({
        queryKey: ["properties", propertyId, "units"],
      });
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Panel>
      <PanelHeader title={kind.label} description={HOW[kind.mode]} />
      <div className="flex flex-wrap items-end gap-3 p-5 pt-2">
        {manage && (
          <>
            <F label="Property type" className="min-w-52">
              <select
                className={input}
                value={value}
                onChange={(e) => setPick(e.target.value)}
              >
                {KINDS.map((k) => (
                  <option key={k.key} value={k.key}>
                    {k.label}
                  </option>
                ))}
              </select>
            </F>
            {pick && pick !== kind.key && (
              <Button size="sm" loading={busy} onClick={() => void save()}>
                Save
              </Button>
            )}
          </>
        )}
        {kind.mode === "sites" && (
          <Button size="sm" variant="secondary" asChild>
            <Link href="/console/campground">Open campgrounds</Link>
          </Button>
        )}
      </div>
    </Panel>
  );
}
