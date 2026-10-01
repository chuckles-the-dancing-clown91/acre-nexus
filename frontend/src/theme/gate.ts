// Server-side gate resolution: which workspace and audience this host serves,
// and that workspace's branding. Server components only (reads request headers).

import { cookies, headers } from "next/headers";
import type { PublicTheme } from "@/lib/types";
import type { ResolveResult } from "@/lib/api";
import { DEFAULT_ACCENT } from "./accent";
import {
  HUD_COOKIE,
  normalizeAudience,
  type Audience,
  type Brand,
  type Gate,
} from "./themes";

const API =
  process.env.API_INTERNAL_URL ??
  process.env.NEXT_PUBLIC_API_URL ??
  "http://localhost:8000";
const DEFAULT_TENANT = process.env.NEXT_PUBLIC_DEFAULT_TENANT ?? "northwind";
// Local development has no host mapping, so the gate needs to be told which
// surface localhost represents.
const DEFAULT_AUDIENCE = normalizeAudience(
  process.env.NEXT_PUBLIC_DEFAULT_AUDIENCE ?? "admin"
);
const REVALIDATE = 60;

const FALLBACK_BRAND: Brand = {
  company_name: "Vantedge",
  logo_url: null,
  accent_color: DEFAULT_ACCENT,
};

async function getJson<T>(path: string, tenant?: string): Promise<T | null> {
  try {
    const r = await fetch(`${API}${path}`, {
      headers: tenant ? { "X-Tenant": tenant } : undefined,
      next: { revalidate: REVALIDATE },
    });
    return r.ok ? ((await r.json()) as T) : null;
  } catch {
    return null;
  }
}

async function brandFor(tenant: string): Promise<Brand> {
  const t = await getJson<PublicTheme>("/public/theme", tenant);
  if (!t) return FALLBACK_BRAND;
  return {
    company_name: t.company_name || FALLBACK_BRAND.company_name,
    logo_url: t.logo_url,
    accent_color: t.accent_color || t.primary_color || DEFAULT_ACCENT,
  };
}

function isLocal(host: string): boolean {
  return (
    !host ||
    host === "localhost" ||
    host === "0.0.0.0" ||
    host.startsWith("127.") ||
    host.endsWith(".localhost")
  );
}

export async function resolveGate(): Promise<Gate> {
  const h = await headers();
  const host = (h.get("x-forwarded-host") ?? h.get("host") ?? "")
    .split(":")[0]
    .toLowerCase();

  let tenant = DEFAULT_TENANT;
  let audience: Audience = isLocal(host) ? DEFAULT_AUDIENCE : "public";
  if (!isLocal(host)) {
    const r = await getJson<ResolveResult>(
      `/public/resolve?host=${encodeURIComponent(host)}`
    );
    if (r) {
      tenant = r.tenant_slug;
      audience = normalizeAudience(r.audience);
    }
  }
  return { tenant, audience, brand: await brandFor(tenant) };
}

export async function hudPreference(): Promise<boolean> {
  return (await cookies()).get(HUD_COOKIE)?.value === "1";
}
