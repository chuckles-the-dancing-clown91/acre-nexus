"use client";

// Everything about one appliance that a person types: what it is, when it was
// bought, and the warranty details a claim needs.

import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api } from "@/lib/api";
import type { Asset } from "@/lib/types";
import { F, FormDialog, input, why } from "@/components/property/bits";

const KINDS: [string, string][] = [
  ["appliance", "Appliance"],
  ["hvac", "Heating and cooling"],
  ["plumbing", "Plumbing"],
  ["electrical", "Electrical"],
  ["safety", "Safety"],
  ["structural", "Structure"],
  ["other", "Other"],
];

const d10 = (v: string | null) => v?.slice(0, 10) ?? "";

export function EditAppliance({
  asset,
  open,
  onOpenChange,
}: {
  asset: Asset;
  open: boolean;
  onOpenChange: (o: boolean) => void;
}) {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [f, setF] = useState(() => ({
    kind: asset.kind,
    name: asset.name,
    make: asset.make ?? "",
    model: asset.model ?? "",
    serial_number: asset.serial_number ?? "",
    location: asset.location ?? "",
    install_date: d10(asset.install_date),
    purchased_on: d10(asset.purchased_on),
    price:
      asset.purchase_price_cents != null
        ? String(asset.purchase_price_cents / 100)
        : "",
    life: asset.expected_life_years ? String(asset.expected_life_years) : "",
    manual_url: asset.manual_url ?? "",
    warranty_provider: asset.warranty_provider ?? "",
    warranty_policy_number: asset.warranty_policy_number ?? "",
    warranty_phone: asset.warranty_phone ?? "",
    warranty_starts_on: d10(asset.warranty_starts_on),
    warranty_expires: d10(asset.warranty_expires),
    warranty_coverage: asset.warranty_coverage ?? "",
    warranty_transferable: asset.warranty_transferable,
    notes: asset.notes ?? "",
  }));
  const set = (k: keyof typeof f, v: string | boolean) =>
    setF((c) => ({ ...c, [k]: v }));
  const text = (k: keyof typeof f) => ({
    className: input,
    value: f[k] as string,
    onChange: (e: React.ChangeEvent<HTMLInputElement>) =>
      set(k, e.target.value),
  });

  async function save() {
    if (!f.name.trim()) return void toast.error("Name it first");
    const price = f.price.trim() === "" ? undefined : Number(f.price);
    if (price !== undefined && (Number.isNaN(price) || price < 0))
      return void toast.error("Price should be a number");
    setBusy(true);
    try {
      await api.updateAsset(asset.id, {
        kind: f.kind,
        name: f.name.trim(),
        make: f.make,
        model: f.model,
        serial_number: f.serial_number,
        location: f.location,
        install_date: f.install_date,
        purchased_on: f.purchased_on,
        purchase_price_cents:
          price === undefined ? undefined : Math.round(price * 100),
        expected_life_years: f.life ? Number(f.life) : undefined,
        manual_url: f.manual_url,
        warranty_provider: f.warranty_provider,
        warranty_policy_number: f.warranty_policy_number,
        warranty_phone: f.warranty_phone,
        warranty_starts_on: f.warranty_starts_on,
        warranty_expires: f.warranty_expires,
        warranty_coverage: f.warranty_coverage,
        warranty_transferable: f.warranty_transferable,
        notes: f.notes,
      });
      toast.success("Saved");
      onOpenChange(false);
      void qc.invalidateQueries({ queryKey: ["asset"] });
      void qc.invalidateQueries({ queryKey: ["assets"] });
    } catch (e) {
      toast.error(why(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <FormDialog
      open={open}
      onOpenChange={onOpenChange}
      title="Edit appliance"
      busy={busy}
      wide
      onSave={() => void save()}
    >
      <F label="Name" className="sm:col-span-2">
        <input {...text("name")} />
      </F>
      <F label="Type">
        <select
          className={input}
          value={f.kind}
          onChange={(e) => set("kind", e.target.value)}
        >
          {KINDS.map(([k, l]) => (
            <option key={k} value={k}>
              {l}
            </option>
          ))}
        </select>
      </F>
      <F label="Where it is">
        <input {...text("location")} placeholder="Garage" />
      </F>
      <F label="Make">
        <input {...text("make")} />
      </F>
      <F label="Model">
        <input {...text("model")} />
      </F>
      <F label="Serial number">
        <input {...text("serial_number")} />
      </F>
      <F label="Link to the manual">
        <input {...text("manual_url")} placeholder="https://" />
      </F>
      <F label="Installed">
        <input type="date" {...text("install_date")} />
      </F>
      <F label="Bought">
        <input type="date" {...text("purchased_on")} />
      </F>
      <F label="Price ($)">
        <input {...text("price")} inputMode="decimal" />
      </F>
      <F label="Expected life (years)">
        <input {...text("life")} inputMode="numeric" />
      </F>

      <div className="eyebrow sm:col-span-2 mt-2">Warranty</div>
      <F label="Provider">
        <input {...text("warranty_provider")} />
      </F>
      <F label="Policy or registration number">
        <input {...text("warranty_policy_number")} />
      </F>
      <F label="Claims phone">
        <input {...text("warranty_phone")} inputMode="tel" />
      </F>
      <label className="flex items-center gap-2 self-end pb-2 text-[13px] text-fg">
        <input
          type="checkbox"
          checked={f.warranty_transferable}
          onChange={(e) => set("warranty_transferable", e.target.checked)}
        />
        Transfers to a new owner
      </label>
      <F label="Starts">
        <input type="date" {...text("warranty_starts_on")} />
      </F>
      <F label="Ends">
        <input type="date" {...text("warranty_expires")} />
      </F>
      <F label="What it covers" className="sm:col-span-2">
        <input
          {...text("warranty_coverage")}
          placeholder="Parts 6 years, labor 1 year"
        />
      </F>
      <F label="Notes" className="sm:col-span-2">
        <textarea
          className={`${input} min-h-20`}
          value={f.notes}
          onChange={(e) => set("notes", e.target.value)}
        />
      </F>
    </FormDialog>
  );
}
