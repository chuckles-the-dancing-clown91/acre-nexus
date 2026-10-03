// Listing syndication: the portals' feeds and what keeps a listing off them.

import {
  API_BASE,
  ApiError,
  actingTenant,
  api,
  request,
  tokenStore,
} from "@/lib/api";

export interface Issue {
  message: string;
  blocking: boolean;
}

export interface Channel {
  key: "zillow" | "mits";
  label: string;
  reaches: string;
  format: string;
  how_to: string;
  enabled: boolean;
  feed_url: string;
  contact_name: string | null;
  contact_email: string | null;
  contact_phone: string | null;
  shown_name: string;
  shown_email: string;
  shown_phone: string;
  company: string;
  issues: Issue[];
  listings: number;
  last_pulled_at: string | null;
  last_pull_agent: string | null;
  pull_count: number;
  pulls: { at: string; agent: string | null; listings: number }[];
}

export interface ListingSyndication {
  id: string;
  property_id: string | null;
  title: string;
  address: string;
  city: string;
  state: string;
  postal_code: string;
  rent_label: string;
  status: string;
  is_public: boolean;
  syndicate: boolean;
  photos: number;
  ready: boolean;
  issues: Issue[];
}

export interface Syndication {
  channels: Channel[];
  listings: ListingSyndication[];
}

export const syndication = {
  overview: () => request<Syndication>("/syndication", { auth: true }),
  update: (
    key: string,
    body: Partial<{
      enabled: boolean;
      contact_name: string;
      contact_email: string;
      contact_phone: string;
    }>
  ) =>
    request<Channel>(`/syndication/${key}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  rotate: (key: string) =>
    request<Channel>(`/syndication/${key}/rotate`, {
      method: "POST",
      auth: true,
    }),
  /** The feed as the portal would get it. */
  preview: async (key: string): Promise<string> => {
    const headers: Record<string, string> = {};
    const token = tokenStore.access;
    if (token) headers["Authorization"] = `Bearer ${token}`;
    const acting = actingTenant.get();
    if (acting) headers["X-Tenant"] = acting;
    const res = await fetch(`${API_BASE}/syndication/${key}/preview`, {
      headers,
      cache: "no-store",
    });
    if (!res.ok) throw new ApiError(res.status, "error", res.statusText);
    return res.text();
  },
};

/** "2 hours ago", "3 days ago", "just now". */
export function since(iso: string, now = Date.now()): string {
  const m = Math.round((now - new Date(iso).getTime()) / 60_000);
  if (m < 1) return "just now";
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} hour${h === 1 ? "" : "s"} ago`;
  const d = Math.round(h / 24);
  return `${d} day${d === 1 ? "" : "s"} ago`;
}

export interface ListingPhoto {
  id: string;
  document_id: string;
  alt_text: string;
  caption: string | null;
  position: number;
  public_url: string;
  preview_url: string | null;
}

export const listingPhotos = {
  list: (listingId: string) =>
    request<ListingPhoto[]>(`/listings/${listingId}/photos`, { auth: true }),
  /** Upload the image (filed on the listing), then put it on the listing. */
  add: async (listingId: string, file: File, alt: string) => {
    const doc = await api.uploadDocument(
      {
        owner_type: "listing",
        owner_id: listingId,
        filename: file.name,
        mime_type: file.type || "image/jpeg",
        category: "photo",
      },
      file
    );
    return request<ListingPhoto[]>(`/listings/${listingId}/photos`, {
      method: "POST",
      auth: true,
      body: { document_id: doc.id, alt_text: alt },
    });
  },
  update: (photoId: string, body: { alt_text?: string; caption?: string }) =>
    request<ListingPhoto[]>(`/listing-photos/${photoId}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  remove: (photoId: string) =>
    request<ListingPhoto[]>(`/listing-photos/${photoId}`, {
      method: "DELETE",
      auth: true,
    }),
  order: (listingId: string, ids: string[]) =>
    request<ListingPhoto[]>(`/listings/${listingId}/photos/order`, {
      method: "PUT",
      auth: true,
      body: { ids },
    }),
};

/** "IMG_2041-living-room.jpg" → "Living room". */
export function altFromFilename(name: string, fallback: string): string {
  const base = name
    .replace(/\.[a-z0-9]+$/i, "")
    .replace(/^(img|dsc|pxl|photo|image)[_-]?\d*[_-]?/i, "")
    .replace(/[_-]+/g, " ")
    .replace(/\d{6,}/g, "")
    .trim();
  return base.length >= 3 ? base[0].toUpperCase() + base.slice(1) : fallback;
}
