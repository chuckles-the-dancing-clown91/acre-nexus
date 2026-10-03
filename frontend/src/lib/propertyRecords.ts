// The records behind a full property profile: permits, insurance policies,
// the schools it's zoned for, action items, and what needs attention.

import { request } from "@/lib/api";

export const PERMIT_KINDS = [
  "building",
  "electrical",
  "plumbing",
  "mechanical",
  "roofing",
  "demolition",
  "fence",
  "solar",
  "pool",
  "other",
] as const;

export const PERMIT_STATUSES = [
  "applied",
  "issued",
  "inspection",
  "finaled",
  "expired",
  "void",
] as const;

export const POLICY_KINDS = [
  "property",
  "liability",
  "flood",
  "earthquake",
  "umbrella",
  "builders_risk",
  "rent_loss",
  "other",
] as const;

export const SCHOOL_LEVELS = [
  "preschool",
  "elementary",
  "middle",
  "high",
  "k8",
  "k12",
  "other",
] as const;

/** Document categories that are drawings of the property. */
export const PLAN_CATEGORIES = [
  "floorplan",
  "blueprint",
  "survey",
  "permit",
] as const;

export interface Permit {
  id: string;
  property_id: string;
  unit_id: string | null;
  permit_number: string | null;
  kind: string;
  description: string;
  status: string;
  open: boolean;
  jurisdiction: string | null;
  applied_on: string | null;
  issued_on: string | null;
  expires_on: string | null;
  inspection_on: string | null;
  finaled_on: string | null;
  contractor_entity_id: string | null;
  contractor_name: string | null;
  valuation_cents: number | null;
  valuation_label: string | null;
  fee_cents: number | null;
  fee_label: string | null;
  ticket_id: string | null;
  document_ids: string[];
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export type PermitInput = {
  description: string;
  kind?: string;
  status?: string;
  permit_number?: string;
  jurisdiction?: string;
  applied_on?: string;
  issued_on?: string;
  expires_on?: string;
  inspection_on?: string;
  finaled_on?: string;
  contractor_name?: string;
  valuation_cents?: number | null;
  fee_cents?: number | null;
  document_ids?: string[];
  notes?: string;
};

export interface Policy {
  id: string;
  property_id: string;
  kind: string;
  carrier: string;
  policy_number: string | null;
  status: string;
  effective_on: string | null;
  expires_on: string | null;
  premium_cents: number | null;
  premium_label: string | null;
  coverage_cents: number | null;
  coverage_label: string | null;
  deductible_cents: number | null;
  deductible_label: string | null;
  agent_name: string | null;
  agent_phone: string | null;
  agent_email: string | null;
  document_ids: string[];
  notes: string | null;
}

export type PolicyInput = {
  carrier: string;
  kind?: string;
  status?: string;
  policy_number?: string;
  effective_on?: string;
  expires_on?: string;
  premium_cents?: number | null;
  coverage_cents?: number | null;
  deductible_cents?: number | null;
  agent_name?: string;
  agent_phone?: string;
  agent_email?: string;
  document_ids?: string[];
  notes?: string;
};

export interface SchoolRecord {
  id: string;
  property_id: string;
  name: string;
  level: string;
  district: string | null;
  grades: string | null;
  rating: number | null;
  distance_mi: number | null;
  assigned: boolean;
  zone_name: string | null;
  zone_verified_on: string | null;
  address: string | null;
  phone: string | null;
  website: string | null;
  enrollment: number | null;
  notes: string | null;
  source: string;
  simulated: boolean;
}

export type SchoolInput = {
  name: string;
  level: string;
  district?: string;
  grades?: string;
  rating?: number | null;
  distance_mi?: number | null;
  assigned: boolean;
  zone_name?: string;
  zone_verified_on?: string;
  address?: string;
  phone?: string;
  website?: string;
  enrollment?: number | null;
  notes?: string;
};

export interface ActionItem {
  id: string;
  property_id: string;
  subject_type: string;
  subject_id: string | null;
  title: string;
  notes: string | null;
  due_on: string | null;
  priority: "low" | "normal" | "high";
  status: "open" | "done" | "dismissed";
  overdue: boolean;
  assignee_user_id: string | null;
  assignee_name: string | null;
  suggestion_key: string | null;
  completed_at: string | null;
  created_at: string;
}

export interface Suggestion {
  key: string;
  subject_type: string;
  subject_id: string | null;
  title: string;
  detail: string;
  due_on: string | null;
  priority: "low" | "normal" | "high";
}

const base = (id: string) => `/properties/${id}`;
const send = <T>(method: string, path: string, body?: unknown) =>
  request<T>(path, { method, auth: true, body });

export const records = {
  permits: (id: string) =>
    request<Permit[]>(`${base(id)}/permits`, { auth: true }),
  createPermit: (id: string, body: PermitInput) =>
    send<Permit>("POST", `${base(id)}/permits`, body),
  updatePermit: (id: string, permitId: string, body: PermitInput) =>
    send<Permit>("PUT", `${base(id)}/permits/${permitId}`, body),
  deletePermit: (id: string, permitId: string) =>
    send<{ deleted: boolean }>("DELETE", `${base(id)}/permits/${permitId}`),

  policies: (id: string) =>
    request<Policy[]>(`${base(id)}/insurance`, { auth: true }),
  createPolicy: (id: string, body: PolicyInput) =>
    send<Policy>("POST", `${base(id)}/insurance`, body),
  updatePolicy: (id: string, policyId: string, body: PolicyInput) =>
    send<Policy>("PUT", `${base(id)}/insurance/${policyId}`, body),
  deletePolicy: (id: string, policyId: string) =>
    send<{ deleted: boolean }>("DELETE", `${base(id)}/insurance/${policyId}`),

  schools: (id: string) =>
    request<SchoolRecord[]>(`${base(id)}/schools`, { auth: true }),
  createSchool: (id: string, body: SchoolInput) =>
    send<SchoolRecord>("POST", `${base(id)}/schools`, body),
  updateSchool: (id: string, schoolId: string, body: SchoolInput) =>
    send<SchoolRecord>("PUT", `${base(id)}/schools/${schoolId}`, body),
  deleteSchool: (id: string, schoolId: string) =>
    send<{ deleted: boolean }>("DELETE", `${base(id)}/schools/${schoolId}`),

  actionItems: (id: string, status: "open" | "done" | "all" = "open") =>
    request<ActionItem[]>(`${base(id)}/action-items?status=${status}`, {
      auth: true,
    }),
  createActionItem: (
    id: string,
    body: {
      title: string;
      subject_type?: string;
      subject_id?: string | null;
      notes?: string;
      due_on?: string | null;
      priority?: string;
      suggestion_key?: string;
    }
  ) => send<ActionItem>("POST", `${base(id)}/action-items`, body),
  updateActionItem: (
    id: string,
    itemId: string,
    body: Partial<{
      title: string;
      notes: string;
      due_on: string;
      priority: string;
      status: string;
    }>
  ) => send<ActionItem>("PATCH", `${base(id)}/action-items/${itemId}`, body),
  deleteActionItem: (id: string, itemId: string) =>
    send<{ deleted: boolean }>("DELETE", `${base(id)}/action-items/${itemId}`),

  attention: (id: string) =>
    request<Suggestion[]>(`${base(id)}/attention`, { auth: true }),
};

/** "builders_risk" → "Builders risk"; "k8" → "K-8". */
export function label(v: string): string {
  if (v === "k8") return "K-8";
  if (v === "k12") return "K-12";
  if (v === "hvac") return "HVAC";
  const s = v.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** "2026-03-05" → "Mar 5, 2026", read as a calendar day (no time zone shift). */
export function day(iso: string | null | undefined): string {
  if (!iso) return "";
  const [y, m, d] = iso.split("-").map(Number);
  if (!y || !m || !d) return iso;
  return new Date(y, m - 1, d).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

/** Days from `today` to `iso` (negative when past). */
export function daysUntil(iso: string, today = new Date()): number {
  const [y, m, d] = iso.split("-").map(Number);
  const then = Date.UTC(y, m - 1, d);
  const now = Date.UTC(today.getFullYear(), today.getMonth(), today.getDate());
  return Math.round((then - now) / 86_400_000);
}

/** "Due in 3 days", "Due today", "4 days late". */
export function dueLabel(iso: string | null, today = new Date()): string {
  if (!iso) return "";
  const n = daysUntil(iso, today);
  if (n === 0) return "Due today";
  if (n === 1) return "Due tomorrow";
  if (n > 1 && n <= 14) return `Due in ${n} days`;
  if (n > 14) return `Due ${day(iso)}`;
  return n === -1 ? "1 day late" : `${-n} days late`;
}

/** The job kit for replacing a piece of equipment, by what it's called. */
export function replacementKit(name: string): string | null {
  const n = name.toLowerCase();
  const rules: [RegExp, string][] = [
    [/dish ?washer/, "replace-dishwasher"],
    [/water heater|hot water/, "replace-water-heater"],
    [/thermostat/, "replace-thermostat"],
    [/fridge|refrigerator/, "replace-refrigerator"],
    [/range|stove|oven|cooktop/, "replace-range"],
    [/disposal/, "replace-garbage-disposal"],
    [/toilet/, "replace-toilet"],
    [/smoke|carbon monoxide|\bco\b/, "replace-smoke-co-detectors"],
    [
      /furnace|hvac|heat pump|air handler|condenser|\bac\b|a\/c|air condition/,
      "service-hvac",
    ],
    [/ceiling fan/, "install-ceiling-fan"],
    [/faucet/, "replace-kitchen-faucet"],
    [/blind/, "replace-blinds"],
  ];
  return rules.find(([re]) => re.test(n))?.[1] ?? null;
}

/** Whole dollars typed in a form → cents; blank → null. */
export function centsFrom(raw: string): number | null {
  const v = Number(raw.replace(/[$,\s]/g, ""));
  if (!raw.trim() || !Number.isFinite(v) || v < 0) return null;
  return Math.round(v * 100);
}

/** Cents → what goes back in a form field. */
export function dollarsField(cents: number | null | undefined): string {
  return cents == null ? "" : String(cents / 100);
}
