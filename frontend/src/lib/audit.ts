// The audit trail: who changed what, on which property (roadmap area 1).

import { request } from "@/lib/api";

export interface FieldChange {
  field: string;
  from: unknown;
  to: unknown;
}

export interface TrailEvent {
  id: string;
  at: string;
  actor_id: string | null;
  actor_name: string;
  /** A Vantedge employee made this change on the customer's behalf. */
  support: boolean;
  action: string;
  target_type: string | null;
  target_id: string | null;
  property_id: string | null;
  label: string;
  summary: string;
  changes: FieldChange[];
  reason: string | null;
}

export interface TrailPage {
  events: TrailEvent[];
  next: string | null;
}

export interface TrailFilter {
  property_id?: string;
  target_type?: string;
  actor?: string;
  from?: string;
  to?: string;
  support?: boolean;
  before?: string;
  limit?: number;
}

function qs(f: Record<string, string | number | boolean | undefined>) {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(f)) {
    if (v === undefined || v === "" || v === false) continue;
    p.set(k, String(v));
  }
  const s = p.toString();
  return s ? `?${s}` : "";
}

export const audit = {
  propertyHistory: (
    id: string,
    f: {
      kind?: string;
      support?: boolean;
      before?: string;
      limit?: number;
    } = {}
  ) => request<TrailPage>(`/properties/${id}/history${qs(f)}`, { auth: true }),
  events: (f: TrailFilter = {}) =>
    request<TrailPage>(`/audit/events${qs({ ...f })}`, { auth: true }),
  csvPath: (f: TrailFilter = {}) =>
    `/audit/events.csv${qs({ ...f, before: undefined, limit: undefined })}`,
};

/** What kinds of record a history can be narrowed to. */
export const KINDS: { value: string; label: string }[] = [
  { value: "", label: "Everything" },
  { value: "property", label: "The property" },
  { value: "unit", label: "Units" },
  { value: "asset", label: "Appliances" },
  { value: "maintenance_ticket", label: "Work orders" },
  { value: "lease", label: "Leases" },
  { value: "listing", label: "Listings" },
  { value: "site_map", label: "Site maps" },
  { value: "process", label: "Turnovers" },
];

/** `market_rent_cents` → "Market rent". */
export function fieldLabel(field: string): string {
  const s = field
    .replace(/_cents$/, "")
    .replace(/_/g, " ")
    .trim();
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** A value as a person would read it: money for `_cents`, dates trimmed. */
export function showValue(field: string, v: unknown): string {
  if (v === null || v === undefined || v === "") return "—";
  if (typeof v === "boolean") return v ? "yes" : "no";
  if (field.endsWith("_cents") && typeof v === "number")
    return `$${(v / 100).toLocaleString(undefined, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    })}`;
  if (typeof v === "string" && /^\d{4}-\d{2}-\d{2}T/.test(v))
    return v.slice(0, 16).replace("T", " ");
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}
