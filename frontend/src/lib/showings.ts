// Showings on a phone: a landlord walks a prospect through, then sends the
// application and, once approved, the lease to sign, without a desk.

import { request } from "@/lib/api";
import type { Lead } from "@/lib/api";

export interface InviteResult {
  lead: Lead;
  /** The link they were sent, to text or show as well. */
  apply_url: string;
}

export const showings = {
  /** Email and text a prospect the application link. */
  invite: (leadId: string, body: { listing_id?: string; message?: string }) =>
    request<InviteResult>(`/leads/${leadId}/invite`, {
      method: "POST",
      auth: true,
      body,
    }),
};

/** "tel:" and "sms:" links for a phone number, or none. */
export function phoneLinks(phone: string | null | undefined): {
  tel: string;
  sms: string;
} | null {
  const digits = (phone ?? "").replace(/[^\d+]/g, "");
  if (digits.length < 7) return null;
  return { tel: `tel:${digits}`, sms: `sms:${digits}` };
}

/** A maps link for an address. */
export function mapLink(address: string): string {
  return `https://maps.google.com/?q=${encodeURIComponent(address)}`;
}

/** What a lead's pipeline status means on the page. */
export function leadStage(l: Lead): string {
  switch (l.status) {
    case "new":
      return "New";
    case "contacted":
      return "Reached out";
    case "toured":
      return "Toured";
    case "applied":
      return "Applied";
    case "closed":
      return "Closed";
    default:
      return l.status;
  }
}

/** Dollars typed by hand → cents, or null when it isn't a number. */
export function dollarsToCents(text: string): number | null {
  const clean = text.replace(/[$,\s]/g, "");
  if (!/^\d+(\.\d{1,2})?$/.test(clean)) return null;
  const [whole, frac = ""] = clean.split(".");
  return Number(whole) * 100 + Number((frac + "00").slice(0, 2));
}
