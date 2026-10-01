// Vendor compliance: the W-9 (taxpayer id, only the last four shown) and
// insurance certificates with their end dates.

import { ApiError, request } from "@/lib/api";

export interface W9 {
  legal_name: string;
  business_name: string | null;
  classification: string;
  tin_type: "ssn" | "ein";
  tin_masked: string;
  signed_on: string | null;
  document_id: string | null;
  updated_at: string;
}

export interface Insurance {
  id: string;
  kind: string;
  carrier: string;
  policy_number: string | null;
  limit_cents: number | null;
  limit_label: string | null;
  expires_on: string;
  state: "current" | "expiring" | "expired";
  document_id: string | null;
}

export interface Compliance {
  counterparty_id: string;
  name: string;
  w9: W9 | null;
  insurance: Insurance[];
  coi_current: boolean;
  problems: string[];
}

export interface VendorAttention {
  counterparty_id: string;
  name: string;
  paid_this_year_cents: number;
  paid_this_year_label: string;
  problems: string[];
}

export const CLASSIFICATIONS: [string, string][] = [
  ["individual", "Individual or sole proprietor"],
  ["c_corp", "C corporation"],
  ["s_corp", "S corporation"],
  ["partnership", "Partnership"],
  ["trust", "Trust or estate"],
  ["llc_c", "LLC taxed as C corp"],
  ["llc_s", "LLC taxed as S corp"],
  ["llc_p", "LLC taxed as partnership"],
  ["other", "Other"],
];

export const INSURANCE_KINDS: [string, string][] = [
  ["general_liability", "General liability"],
  ["workers_comp", "Workers' comp"],
  ["auto", "Auto"],
  ["umbrella", "Umbrella"],
  ["professional", "Professional"],
];

/**
 * Send a vendor out. When the workspace requires current insurance and this
 * vendor has none, the API answers 409; ask for a reason and try once more
 * with it. Resolves to `null` when the person backs out.
 */
export async function withCoiOverride<T>(
  send: (reason?: string) => Promise<T>
): Promise<T | null> {
  try {
    return await send();
  } catch (e) {
    if (!(e instanceof ApiError) || e.status !== 409) throw e;
    const reason = window.prompt(`${e.message}\n\nReason to send them anyway:`);
    if (!reason?.trim()) return null;
    return send(reason.trim());
  }
}

export const vendors = {
  compliance: (id: string) =>
    request<Compliance>(`/entities/${id}/compliance`, { auth: true }),
  saveW9: (
    id: string,
    body: {
      legal_name: string;
      business_name?: string;
      classification: string;
      tin_type: string;
      tin: string;
      signed_on?: string;
    }
  ) =>
    request<Compliance>(`/entities/${id}/w9`, {
      method: "PUT",
      auth: true,
      body,
    }),
  addInsurance: (
    id: string,
    body: {
      kind: string;
      carrier: string;
      policy_number?: string;
      limit_cents?: number;
      expires_on: string;
    }
  ) =>
    request<Compliance>(`/entities/${id}/insurance`, {
      method: "POST",
      auth: true,
      body,
    }),
  removeInsurance: (certId: string) =>
    request<Compliance>(`/vendor-insurance/${certId}`, {
      method: "DELETE",
      auth: true,
    }),
  attention: () =>
    request<VendorAttention[]>("/compliance/vendors", { auth: true }),
};
