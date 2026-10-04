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

/** The job API, from a vendor's link (`/public/vendor/<token>`, no sign-in)
 * or from the vendor portal (`/vendor-portal/jobs/<batch>`, signed in). */
export function vendorClient(base: string, auth: boolean) {
  const post = <T>(path: string, body: unknown) =>
    request<T>(`${base}${path}`, { method: "POST", body, auth });
  return {
    view: () => request<VendorJob>(base, { auth }),
    accept: (body: { note?: string; start?: string; end?: string }) =>
      post<VendorJob>("/accept", body),
    decline: (reason?: string) => post<VendorJob>("/decline", { reason }),
    done: (note?: string) => post<VendorJob>("/done", { note }),
    invoice: (body: {
      amount_cents: number;
      description?: string;
      document_id?: string;
    }) => post<VendorJob>("/invoice", body),
    upload: async (
      file: File,
      kind: "photo" | "invoice"
    ): Promise<VendorFile> => {
      const reg = await post<{ file: VendorFile; upload_url: string }>(
        "/uploads",
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
}

export type VendorClient = ReturnType<typeof vendorClient>;

/** A vendor's link. */
export const linkClient = (token: string) =>
  vendorClient(`/public/vendor/${token}`, false);

/** A job in the signed-in vendor portal. */
export const portalClient = (batch: string) =>
  vendorClient(`/vendor-portal/jobs/${batch}`, true);

export interface PortalJobRow {
  batch: string;
  ticket_id: string;
  title: string;
  property: string;
  priority: string;
  due_date: string | null;
  tasks: number;
  tasks_done: number;
  response: string | null;
  sent_at: string | null;
  open: boolean;
}

export const vendorPortal = {
  me: () =>
    request<{
      vendor_id: string;
      vendor: string;
      company: string;
      open_jobs: number;
    }>("/vendor-portal/me", { auth: true }),
  jobs: () =>
    request<{ open: PortalJobRow[]; closed: PortalJobRow[] }>(
      "/vendor-portal/jobs",
      { auth: true }
    ),
  invite: (entityId: string) =>
    request<{
      entity_id: string;
      user_id: string;
      outcome: "invited" | "linked";
    }>(`/entities/${entityId}/portal-invite`, { method: "POST", auth: true }),
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
