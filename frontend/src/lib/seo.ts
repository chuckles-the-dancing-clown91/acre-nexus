// Server-side helpers for the public site's search markup: which workspace
// this host serves, where the site lives, and cached reads of public data.
// Server components only.

import { headers } from "next/headers";
import type { Listing } from "@/lib/types";
import type { SiteInfo } from "@/lib/seo-schema";

const API =
  process.env.API_INTERNAL_URL ??
  process.env.NEXT_PUBLIC_API_URL ??
  "http://localhost:8000";
const DEFAULT = process.env.NEXT_PUBLIC_DEFAULT_TENANT ?? "northwind";

/** Public data may be a few minutes old; search engines do not mind. */
const REVALIDATE = 300;

/** The public origin of this site, e.g. `https://homes.example.com`. */
export async function siteOrigin(): Promise<string> {
  const fixed = process.env.NEXT_PUBLIC_SITE_URL;
  if (fixed) return fixed.replace(/\/$/, "");
  const h = await headers();
  const host = h.get("x-forwarded-host") ?? h.get("host") ?? "localhost:3000";
  const proto =
    h.get("x-forwarded-proto") ??
    (host.startsWith("localhost") || host.startsWith("127.")
      ? "http"
      : "https");
  return `${proto}://${host}`;
}

/** The workspace this host serves: a verified custom domain, else the default. */
export async function currentTenant(): Promise<string> {
  const h = await headers();
  const host = (h.get("x-forwarded-host") ?? h.get("host") ?? "")
    .split(":")[0]
    .toLowerCase();
  if (!host || host === "localhost" || host.startsWith("127.")) return DEFAULT;
  try {
    const r = await fetch(
      `${API}/public/resolve?host=${encodeURIComponent(host)}`,
      {
        next: { revalidate: REVALIDATE },
      }
    );
    if (r.ok) {
      const j = (await r.json()) as { tenant_slug?: string };
      if (j.tenant_slug) return j.tenant_slug;
    }
  } catch {
    /* fall through */
  }
  return DEFAULT;
}

/** A cached GET of a public endpoint for a workspace; null when missing. */
export async function publicGet<T>(
  path: string,
  tenant: string
): Promise<T | null> {
  try {
    const r = await fetch(`${API}${path}`, {
      headers: { "X-Tenant": tenant },
      next: { revalidate: REVALIDATE, tags: [`public:${tenant}`] },
    });
    return r.ok ? ((await r.json()) as T) : null;
  } catch {
    return null;
  }
}

export async function loadSite(tenant: string): Promise<SiteInfo> {
  return (
    (await publicGet<SiteInfo>("/public/site", tenant)) ?? {
      slug: tenant,
      company_name: "Vantedge",
      seo_title: null,
      seo_description: null,
      google_site_verification: null,
      phone: null,
      email: null,
      address: null,
      hours: null,
      website: null,
      same_as: [],
      logo_url: null,
    }
  );
}

export async function loadListings(tenant: string): Promise<Listing[]> {
  return (await publicGet<Listing[]>("/public/listings", tenant)) ?? [];
}

export async function loadListing(
  tenant: string,
  id: string
): Promise<Listing | null> {
  return publicGet<Listing>(
    `/public/listings/${encodeURIComponent(id)}`,
    tenant
  );
}
