// The house onboarding checklist and the property record's suggestions.

import { request } from "@/lib/api";

export interface ChecklistStep {
  key: string;
  title: string;
  done: boolean;
  optional: boolean;
  detail: string;
  href: string;
}

export interface Checklist {
  property_id: string;
  steps: ChecklistStep[];
  done: number;
  required: number;
  required_done: number;
  next: string | null;
}

export interface Proposal {
  field: string;
  label: string;
  current: string | null;
  proposed: string;
  source: string;
}

export interface Autofill {
  fetched_at: string | null;
  proposals: Proposal[];
}

export const checklist = {
  get: (id: string) =>
    request<Checklist>(`/properties/${id}/checklist`, { auth: true }),
  autofill: (id: string) =>
    request<Autofill>(`/properties/${id}/autofill`, { auth: true }),
  apply: (id: string, fields: string[]) =>
    request<Autofill>(`/properties/${id}/autofill/apply`, {
      method: "POST",
      auth: true,
      body: { fields },
    }),
};

/** How a suggested value reads. */
export function shown(p: Proposal, v: string | null): string {
  if (v === null) return "not set";
  if (p.field === "unit_market_rent") return `$${Number(v).toLocaleString()}`;
  if (p.field === "property_type") return v.replace(/_/g, " ");
  return v;
}
