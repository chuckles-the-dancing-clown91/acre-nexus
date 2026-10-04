"use client";

// What kind of property a deal is, and for raw land: acreage, zoning, water,
// power and road access, and the price per acre (offer price, else asking).

import { useState } from "react";
import { toast } from "sonner";
import { Trees } from "lucide-react";
import { api, type FlipDeal } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Panel, PanelHeader } from "@/components/ui/panel";
import { F, Fact, input, why } from "@/components/property/bits";

const TYPES = [
  ["single_family", "Single family"],
  ["multifamily", "Multifamily"],
  ["condo", "Condo"],
  ["commercial", "Commercial"],
  ["land", "Raw land"],
] as const;

export function isLand(d: Pick<FlipDeal, "property_type">) {
  return (d.property_type ?? "").toLowerCase() === "land";
}

export function Land({
  deal,
  write,
  onSaved,
}: {
  deal: FlipDeal;
  write: boolean;
  onSaved: () => void;
}) {
  const [f, setF] = useState({
    acres: deal.acres != null ? String(deal.acres) : "",
    zoning: deal.zoning ?? "",
    water_access: deal.water_access ?? "",
    power_access: deal.power_access ?? "",
    road_access: deal.road_access ?? "",
  });
  const [busy, setBusy] = useState(false);
  async function setType(property_type: string) {
    try {
      await api.updateFlipDeal(deal.id, { property_type });
      onSaved();
    } catch (e) {
      toast.error(why(e));
    }
  }
  async function save() {
    const acres = Number(f.acres);
    if (f.acres && !(acres > 0)) {
      toast.error("Acres must be more than zero");
      return;
    }
    setBusy(true);
    try {
      await api.updateFlipDeal(deal.id, {
        ...(f.acres ? { acres } : {}),
        zoning: f.zoning,
        water_access: f.water_access,
        power_access: f.power_access,
        road_access: f.road_access,
      });
      toast.success("Saved");
      onSaved();
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }
  const field = (k: keyof typeof f, label: string, placeholder?: string) => (
    <F label={label}>
      <input
        className={input}
        value={f[k]}
        placeholder={placeholder}
        disabled={!write}
        onChange={(e) => setF({ ...f, [k]: e.target.value })}
      />
    </F>
  );
  const land = isLand(deal);
  const known = TYPES.some(([k]) => k === deal.property_type);
  return (
    <Panel>
      <PanelHeader
        title={land ? "Raw land" : "Property type"}
        description={
          land
            ? "Acreage, zoning, and what reaches the lot."
            : "Pick raw land to track acreage and price per acre."
        }
        action={<Trees className="size-4 text-fg-3" />}
      />
      <div className="space-y-4 p-5 pt-2">
        <F label="Type">
          <select
            className={input}
            value={deal.property_type ?? ""}
            disabled={!write}
            onChange={(e) => void setType(e.target.value)}
          >
            {!deal.property_type && <option value="">Not set</option>}
            {!known && deal.property_type && (
              <option value={deal.property_type}>{deal.property_type}</option>
            )}
            {TYPES.map(([k, label]) => (
              <option key={k} value={k}>
                {label}
              </option>
            ))}
          </select>
        </F>
        {land && (
          <>
            <dl>
              <Fact label="Price per acre">{deal.price_per_acre_label}</Fact>
            </dl>
            <div className="grid gap-3 sm:grid-cols-2">
              {field("acres", "Acres", "12.5")}
              {field("zoning", "Zoning", "A-1 agricultural")}
              {field("water_access", "Water", "Well, city main at road, none")}
              {field("power_access", "Power", "On site, at road, 0.5 mi")}
              {field("road_access", "Road", "Paved frontage, easement")}
            </div>
            {write && (
              <Button size="sm" loading={busy} onClick={() => void save()}>
                Save
              </Button>
            )}
          </>
        )}
      </div>
    </Panel>
  );
}
