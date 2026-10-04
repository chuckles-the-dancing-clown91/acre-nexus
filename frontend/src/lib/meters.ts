// Meters and utilities for a property or one of its units.

import { request } from "@/lib/api";

const auth = { auth: true } as const;

export type MeterKind =
  "electric" | "gas" | "water" | "sewer" | "trash" | "internet" | "other";
export type PaidBy = "tenant" | "landlord" | "shared";

export const METER_KINDS: [MeterKind, string][] = [
  ["electric", "Electricity"],
  ["gas", "Gas"],
  ["water", "Water"],
  ["sewer", "Sewer"],
  ["trash", "Trash and recycling"],
  ["internet", "Internet and cable"],
  ["other", "Other"],
];

export const PAID_BY: [PaidBy, string][] = [
  ["tenant", "Tenant pays"],
  ["landlord", "Landlord pays"],
  ["shared", "Shared"],
];

export interface Meter {
  id: string;
  property_id: string;
  unit_id: string | null;
  kind: MeterKind;
  label: string;
  meter_number: string | null;
  location: string | null;
  provider: string | null;
  unit_of_measure: string | null;
  paid_by: PaidBy;
  billing_note: string | null;
  status: "active" | "retired";
  last_reading: number | null;
  last_read_on: string | null;
}

export interface Reading {
  id: string;
  read_on: string;
  reading: number;
  used: number | null;
  reason: "routine" | "move_in" | "move_out" | "other";
  lease_id: string | null;
  note: string | null;
  created_at: string;
}

export interface UtilityTerm {
  kind: string;
  label: string;
  paid_by: PaidBy;
  paid_by_label: string;
  provider: string | null;
  meters: string[];
  note: string | null;
}

export interface MeterInput {
  property_id: string;
  unit_id?: string;
  kind: MeterKind;
  label?: string;
  meter_number?: string;
  location?: string;
  provider?: string;
  unit_of_measure?: string;
  paid_by?: PaidBy;
  billing_note?: string;
}

export const meters = {
  list: (q: { property_id?: string; unit_id?: string }) => {
    const p = new URLSearchParams();
    if (q.property_id) p.set("property_id", q.property_id);
    if (q.unit_id) p.set("unit_id", q.unit_id);
    return request<Meter[]>(`/meters?${p}`, auth);
  },
  create: (body: MeterInput) =>
    request<Meter>("/meters", { method: "POST", body, ...auth }),
  update: (
    id: string,
    body: Partial<Omit<MeterInput, "property_id" | "unit_id" | "kind">> & {
      status?: "active" | "retired";
    }
  ) => request<Meter>(`/meters/${id}`, { method: "PATCH", body, ...auth }),
  readings: (id: string) => request<Reading[]>(`/meters/${id}/readings`, auth),
  addReading: (
    id: string,
    body: {
      reading: number;
      read_on?: string;
      reason?: Reading["reason"];
      lease_id?: string;
      note?: string;
    }
  ) =>
    request<Reading>(`/meters/${id}/readings`, {
      method: "POST",
      body,
      ...auth,
    }),
  utilities: (propertyId: string, unitId?: string) =>
    request<UtilityTerm[]>(
      `/properties/${propertyId}/utilities${unitId ? `?unit_id=${unitId}` : ""}`,
      auth
    ),
};
