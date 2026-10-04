"use client";

// The selected shape's details: its name, the unit record it stands for, a
// campsite's type, hookups and rates, an amenity's kind, and the tools to
// reshape, delete or repeat it in a row.

import { useState } from "react";
import { Copy, PenLine, Trash2, X } from "lucide-react";
import {
  AMENITIES,
  KIND_LABELS,
  SITE_TYPES,
  featureColor,
  type Feature,
} from "@/lib/sitemaps";
import type { Unit } from "@/lib/types";
import { Badge, statusTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { F, input } from "@/components/property/bits";

const M2_TO_FT2 = 10.7639;
const M_TO_FT = 3.28084;

const SITE_STATUSES = ["available", "booked", "blocked", "out_of_service"];

export function FeaturePanel({
  f,
  write,
  units,
  linked,
  editing,
  onPatch,
  onAttr,
  onEdit,
  onDelete,
  onRow,
  onClose,
}: {
  f: Feature;
  write: boolean;
  units: Unit[];
  linked: Set<string | null>;
  editing: boolean;
  onPatch: (p: Partial<Feature>) => void;
  onAttr: (name: string, v: unknown) => void;
  onEdit: () => void;
  onDelete: () => void;
  onRow: (count: number, stepM: number) => void;
  onClose: () => void;
}) {
  const [count, setCount] = useState(5);
  const [step, setStep] = useState(8);
  const a = f.attrs;
  const num = (k: string) => (a[k] === undefined ? "" : String(a[k]));
  const setNum = (k: string, v: string) =>
    onAttr(k, v === "" ? undefined : Number(v));
  const money = (k: string) =>
    typeof a[k] === "number" ? String((a[k] as number) / 100) : "";
  const setMoney = (k: string, v: string) =>
    onAttr(k, v === "" ? undefined : Math.round(Number(v) * 100));
  const check = (k: string, label: string) => (
    <label
      key={k}
      className="inline-flex items-center gap-1.5 text-xs text-fg-2"
    >
      <input
        type="checkbox"
        className="accent-[var(--accent)]"
        disabled={!write}
        checked={a[k] === true}
        onChange={(e) => onAttr(k, e.target.checked)}
      />
      {label}
    </label>
  );

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <span
          className="size-3 rounded-sm"
          style={{ background: featureColor(f) }}
        />
        <span className="text-[15px] font-semibold text-fg">
          {KIND_LABELS[f.kind]}
        </span>
        {f.unit && (
          <Badge tone={statusTone(f.unit.status)}>
            {f.unit.status.replace(/_/g, " ")}
          </Badge>
        )}
        <Button
          size="icon"
          variant="ghost"
          className="ml-auto size-7"
          aria-label="Close"
          onClick={onClose}
        >
          <X />
        </Button>
      </div>

      <F label="Name">
        <input
          className={input}
          value={f.name ?? ""}
          disabled={!write}
          onChange={(e) => onPatch({ name: e.target.value || null })}
        />
      </F>
      {(f.area_m2 || f.length_m) && (
        <p className="text-xs text-fg-3">
          {f.area_m2
            ? `${Math.round(f.area_m2 * M2_TO_FT2).toLocaleString()} sq ft`
            : `${Math.round((f.length_m ?? 0) * M_TO_FT).toLocaleString()} ft`}
        </p>
      )}

      {f.kind === "unit" && (
        <F label="Unit record">
          <select
            className={input}
            value={f.unit_id ?? ""}
            disabled={!write}
            onChange={(e) => {
              const u = units.find((x) => x.id === e.target.value);
              onPatch({
                unit_id: e.target.value || null,
                name: u ? u.unit_number : f.name,
                unit: u
                  ? {
                      unit_number: u.unit_number,
                      status: u.status,
                      market_rent_label: u.market_rent_label,
                      beds: u.beds,
                      baths: u.baths,
                    }
                  : null,
              });
            }}
          >
            <option value="">Not linked</option>
            {units
              .filter((u) => u.id === f.unit_id || !linked.has(u.id))
              .map((u) => (
                <option key={u.id} value={u.id}>
                  Unit {u.unit_number} ({u.status.replace(/_/g, " ")})
                </option>
              ))}
          </select>
        </F>
      )}
      {f.unit && (
        <p className="text-xs text-fg-2">
          {[
            f.unit.beds ? `${f.unit.beds} bd` : null,
            f.unit.baths ? `${f.unit.baths} ba` : null,
            f.unit.market_rent_label,
          ]
            .filter(Boolean)
            .join(" · ") || "No layout on file"}
        </p>
      )}

      {f.kind === "site" && (
        <div className="space-y-3 border-t border-line pt-4">
          <div className="grid grid-cols-2 gap-2">
            <F label="Type">
              <select
                className={`${input} capitalize`}
                disabled={!write}
                value={String(a.site_type ?? "tent")}
                onChange={(e) => onAttr("site_type", e.target.value)}
              >
                {SITE_TYPES.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
            </F>
            <F label="Status">
              <select
                className={`${input} capitalize`}
                disabled={!write}
                value={String(a.site_status ?? "available")}
                onChange={(e) => onAttr("site_status", e.target.value)}
              >
                {SITE_STATUSES.map((t) => (
                  <option key={t} value={t}>
                    {t.replace(/_/g, " ")}
                  </option>
                ))}
              </select>
            </F>
            <F label="Max length (ft)">
              <input
                className={input}
                type="number"
                min={0}
                disabled={!write}
                value={num("max_length_ft")}
                onChange={(e) => setNum("max_length_ft", e.target.value)}
              />
            </F>
            <F label="Max guests">
              <input
                className={input}
                type="number"
                min={0}
                disabled={!write}
                value={num("max_guests")}
                onChange={(e) => setNum("max_guests", e.target.value)}
              />
            </F>
            <F label="Power" className="col-span-2">
              <select
                className={input}
                disabled={!write}
                value={num("power_amps")}
                onChange={(e) => setNum("power_amps", e.target.value)}
              >
                <option value="">None</option>
                <option value="20">20 amp</option>
                <option value="30">30 amp</option>
                <option value="50">50 amp</option>
              </select>
            </F>
          </div>
          <div className="grid grid-cols-2 gap-1.5">
            {check("pull_through", "Pull-through")}
            {check("water", "Water")}
            {check("sewer", "Sewer")}
            {check("shade", "Shade")}
            {check("ada", "Accessible")}
            {check("pets", "Pets OK")}
          </div>
          <div className="grid grid-cols-3 gap-2">
            {(
              [
                ["rate_cents_night", "Night ($)"],
                ["rate_cents_week", "Week ($)"],
                ["rate_cents_month", "Month ($)"],
              ] as const
            ).map(([k, l]) => (
              <F key={k} label={l}>
                <input
                  className={input}
                  type="number"
                  min={0}
                  disabled={!write}
                  value={money(k)}
                  onChange={(e) => setMoney(k, e.target.value)}
                />
              </F>
            ))}
          </div>
        </div>
      )}

      {f.kind === "amenity" && (
        <F label="Amenity">
          <select
            className={`${input} capitalize`}
            disabled={!write}
            value={String(a.amenity ?? "other")}
            onChange={(e) => onAttr("amenity", e.target.value)}
          >
            {AMENITIES.map((t) => (
              <option key={t} value={t}>
                {t.replace(/_/g, " ")}
              </option>
            ))}
          </select>
        </F>
      )}

      {write && (
        <div className="space-y-3 border-t border-line pt-4">
          <div className="flex gap-2">
            <Button
              size="sm"
              variant={editing ? "primary" : "secondary"}
              onClick={onEdit}
            >
              <PenLine />
              {editing ? "Done reshaping" : "Reshape"}
            </Button>
            <Button size="sm" variant="ghost" onClick={onDelete}>
              <Trash2 />
              Delete
            </Button>
          </div>
          <div className="flex items-end gap-2">
            <F label="Copies">
              <input
                className={`${input} w-16`}
                type="number"
                min={1}
                max={100}
                value={count}
                onChange={(e) => setCount(Number(e.target.value))}
              />
            </F>
            <F label="Every (m)">
              <input
                className={`${input} w-16`}
                type="number"
                min={1}
                value={step}
                onChange={(e) => setStep(Number(e.target.value))}
              />
            </F>
            <Button
              size="sm"
              variant="secondary"
              onClick={() => onRow(Math.max(1, Math.min(100, count)), step)}
            >
              <Copy />
              Add a row
            </Button>
          </div>
          <p className="text-xs text-fg-3">
            A row copies this shape eastward, numbering the names.
          </p>
        </div>
      )}
    </div>
  );
}
