// Public listing search, tour requests, and the autofill review.

import { DEFAULT_TENANT, request } from "@/lib/api";
import type { Listing } from "@/lib/types";

export interface ListingSearch {
  q?: string;
  max_rent?: number;
  beds?: number;
  baths?: number;
  available_now?: boolean;
  sort?: string;
}

export interface TourRequest {
  id: string;
  listing_id: string | null;
  listing_title: string | null;
  name: string;
  email: string;
  phone: string | null;
  preferred_times: string | null;
  message: string | null;
  status: "new" | "contacted" | "scheduled" | "closed";
  note: string | null;
  created_at: string;
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

export const tours = {
  search: (f: ListingSearch, tenant: string = DEFAULT_TENANT) => {
    const p = new URLSearchParams();
    for (const [k, v] of Object.entries(f)) {
      if (v === undefined || v === "" || v === false) continue;
      p.set(k, String(v));
    }
    const s = p.toString();
    return request<Listing[]>(`/public/listings${s ? `?${s}` : ""}`, {
      tenant,
    });
  },
  request: (
    body: {
      listing_id?: string;
      name: string;
      email: string;
      phone?: string;
      preferred_times?: string;
      message?: string;
      consent: boolean;
      website?: string;
    },
    tenant: string = DEFAULT_TENANT
  ) =>
    request<{ ok: boolean }>("/public/tour-requests", {
      method: "POST",
      tenant,
      body,
    }),
  list: (status?: string) =>
    request<TourRequest[]>(
      `/tour-requests${status ? `?status=${status}` : ""}`,
      { auth: true }
    ),
  update: (id: string, body: { status?: string; note?: string }) =>
    request<{ ok: boolean }>(`/tour-requests/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  autofill: (propertyId: string) =>
    request<Autofill>(`/properties/${propertyId}/autofill`, { auth: true }),
  applyAutofill: (propertyId: string, fields: string[]) =>
    request<Autofill>(`/properties/${propertyId}/autofill/apply`, {
      method: "POST",
      auth: true,
      body: { fields },
    }),
};
