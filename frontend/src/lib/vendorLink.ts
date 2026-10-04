// The vendor's link: a work order's tasks, answered without an account.
// Everything here is public and keyed by the token in the link.

import { ApiError, request } from "@/lib/api";

export interface VendorTask {
  id: string;
  title: string;
  trade: string;
  est_minutes: number | null;
  status: string;
}

export interface VendorFile {
  id: string;
  filename: string;
  mime_type: string;
  kind: "photo" | "video" | "receipt" | "document";
  size_bytes: number;
  url: string | null;
  created_at: string;
}

export interface VendorInvoice {
  id: string;
  description: string;
  amount_cents: number;
  amount_label: string;
  incurred_on: string;
}

export interface VendorJob {
  company: string;
  vendor: string;
  title: string;
  description: string | null;
  priority: string;
  property: string;
  access_notes: string | null;
  due_date: string | null;
  note: string | null;
  tasks: VendorTask[];
  response: "accepted" | "declined" | "done" | null;
  responded_at: string | null;
  response_note: string | null;
  when_words: string | null;
  files: VendorFile[];
  invoices: VendorInvoice[];
  timezone: string;
}

const post = <T>(path: string, body: unknown) =>
  request<T>(path, { method: "POST", body });

export const vendorLink = {
  view: (token: string) => request<VendorJob>(`/public/vendor/${token}`),
  accept: (
    token: string,
    body: { note?: string; start?: string; end?: string }
  ) => post<VendorJob>(`/public/vendor/${token}/accept`, body),
  decline: (token: string, reason?: string) =>
    post<VendorJob>(`/public/vendor/${token}/decline`, { reason }),
  done: (token: string, note?: string) =>
    post<VendorJob>(`/public/vendor/${token}/done`, { note }),
  invoice: (
    token: string,
    body: { amount_cents: number; description?: string; document_id?: string }
  ) => post<VendorJob>(`/public/vendor/${token}/invoice`, body),
  upload: async (
    token: string,
    file: File,
    kind: "photo" | "invoice"
  ): Promise<VendorFile> => {
    const reg = await post<{ file: VendorFile; upload_url: string }>(
      `/public/vendor/${token}/uploads`,
      {
        filename: file.name || `${kind}.jpg`,
        mime_type: file.type || "application/octet-stream",
        size_bytes: file.size,
        kind,
      }
    );
    const res = await fetch(reg.upload_url, {
      method: "PUT",
      body: file,
      headers: { "Content-Type": file.type || "application/octet-stream" },
    });
    if (!res.ok)
      throw new ApiError(res.status, "upload_failed", "upload failed");
    return reg.file;
  },
};

/** "$385.00" from cents typed as "385" or "385.5". */
export function centsFrom(text: string): number | null {
  const clean = text.replace(/[$,\s]/g, "");
  if (!/^\d+(\.\d{1,2})?$/.test(clean)) return null;
  const [whole, frac = ""] = clean.split(".");
  return Number(whole) * 100 + Number((frac + "00").slice(0, 2));
}

/** What the vendor's answer means on the work order. */
export function vendorResponseWords(
  r: "accepted" | "declined" | "done" | null
): string | null {
  switch (r) {
    case "accepted":
      return "Accepted";
    case "declined":
      return "Declined";
    case "done":
      return "Vendor says done";
    default:
      return null;
  }
}
