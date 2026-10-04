// Family-plan features (roadmap area 17): related-party reviews and
// foundation mode (vouchers, income certifications, at-cost fee).

import { request } from "@/lib/api";
import type { Counterparty } from "@/lib/types";
import type { LegalEntity } from "@/lib/api";

const auth = { auth: true } as const;

export type ReviewStatus = "open" | "approved" | "rejected";

export interface Review {
  id: string;
  subject_type: "vendor_bill" | "lease" | "deal" | "other";
  subject_id: string | null;
  entity_id: string | null;
  entity_name: string | null;
  counterparty_id: string | null;
  counterparty_name: string | null;
  summary: string;
  reason: string;
  amount_cents: number | null;
  market_cents: number | null;
  market_note: string | null;
  parties: { id: string; name: string }[];
  status: ReviewStatus;
  decided_by: string | null;
  decided_at: string | null;
  decision_note: string | null;
  created_at: string;
  /** Why the signed-in user can't decide it, if they can't. */
  cannot_decide: string | null;
}

export interface FlagInput {
  subject_type: "lease" | "deal" | "other";
  subject_id?: string;
  entity_id?: string;
  counterparty_id?: string;
  summary: string;
  reason?: string;
  amount_cents?: number;
  party_owner_ids?: string[];
}

export interface Voucher {
  authority: string;
  contract_number: string | null;
  hap_cents: number;
  starts_on: string;
  ends_on: string | null;
}

export interface Certification {
  id: string;
  effective_on: string;
  expires_on: string;
  household_size: number;
  annual_income_cents: number;
  ami_cents: number;
  limit_pct: number;
  limit_cents: number;
  qualified: boolean;
  notes: string | null;
  created_at: string;
}

export type CertState =
  "missing" | "ok" | "expiring" | "expired" | "over_limit";

export interface Assistance {
  foundation: boolean;
  rent_cents: number;
  voucher: Voucher | null;
  resident_share_cents: number;
  certifications: Certification[];
  certification: CertState;
  hap_due: { payment_id: string; due_date: string; amount_cents: number }[];
}

export interface ComplianceRow {
  lease_id: string;
  tenant_name: string;
  property_id: string;
  property_name: string;
  entity_name: string;
  certification: CertState;
  expires_on: string | null;
  hap_cents: number | null;
  authority: string | null;
  hap_owed_cents: number;
}

export const family = {
  reviews: (status: ReviewStatus | "all" = "all") =>
    request<Review[]>(`/related-party?status=${status}`, auth),
  flag: (body: FlagInput) =>
    request<Review>("/related-party", { method: "POST", body, ...auth }),
  note: (
    id: string,
    body: { market_cents: number | null; market_note: string | null }
  ) =>
    request<Review>(`/related-party/${id}`, { method: "PATCH", body, ...auth }),
  decide: (id: string, approve: boolean, note?: string) =>
    request<Review>(`/related-party/${id}/decide`, {
      method: "POST",
      body: { approve, note },
      ...auth,
    }),
  setRelated: (
    counterpartyId: string,
    body: { related_llc_id: string | null; related_owner_id: string | null }
  ) =>
    request<Counterparty>(`/entities/${counterpartyId}/related`, {
      method: "PUT",
      body,
      ...auth,
    }),
  setFoundation: (
    llcId: string,
    body: { foundation: boolean; fee_basis: "percent" | "at_cost" }
  ) =>
    request<LegalEntity>(`/llcs/${llcId}/foundation`, {
      method: "PUT",
      body,
      ...auth,
    }),
  assistance: (leaseId: string) =>
    request<Assistance>(`/leases/${leaseId}/assistance`, auth),
  setVoucher: (
    leaseId: string,
    body: {
      authority: string;
      contract_number?: string;
      hap_cents: number;
      starts_on: string;
      ends_on?: string;
    }
  ) =>
    request<Assistance>(`/leases/${leaseId}/voucher`, {
      method: "PUT",
      body,
      ...auth,
    }),
  removeVoucher: (leaseId: string) =>
    request<Assistance>(`/leases/${leaseId}/voucher`, {
      method: "DELETE",
      ...auth,
    }),
  certify: (
    leaseId: string,
    body: {
      effective_on: string;
      expires_on?: string;
      household_size: number;
      annual_income_cents: number;
      ami_cents: number;
      limit_pct: number;
      notes?: string;
    }
  ) =>
    request<Assistance>(`/leases/${leaseId}/income-certifications`, {
      method: "POST",
      body,
      ...auth,
    }),
  compliance: () => request<ComplianceRow[]>("/foundation/compliance", auth),
  hapReceived: (paymentId: string, paid_date?: string) =>
    request<Assistance>(`/lease-payments/${paymentId}/hap-received`, {
      method: "POST",
      body: { paid_date },
      ...auth,
    }),
};

export const CERT_WORDS: Record<CertState, string> = {
  missing: "Not certified",
  ok: "Certified",
  expiring: "Recertify soon",
  expired: "Expired",
  over_limit: "Over the limit",
};

export function certTone(s: CertState): "good" | "warn" | "bad" | "neutral" {
  return s === "ok"
    ? "good"
    : s === "expiring"
      ? "warn"
      : s === "missing"
        ? "neutral"
        : "bad";
}

/** The income limit for a household: `pct` of the area median income. */
export function incomeLimit(amiCents: number, pct: number): number {
  return Math.floor((amiCents * pct) / 100);
}

/** Whether a household's income is within the limit. */
export function qualifies(incomeCents: number, amiCents: number, pct: number) {
  return incomeCents * 100 <= amiCents * pct;
}
