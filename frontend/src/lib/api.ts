// Typed API client for the Acre Rust backend.
//
// Multi-tenancy: public requests carry the tenant via the `X-Tenant` header
// (slug). Authenticated requests carry a JWT `Authorization: Bearer` token;
// platform staff can additionally pass `X-Tenant` to "view as" a client.

import type {
  Application,
  ApplyResponse,
  Listing,
  LlcGroup,
  PortfolioSummary,
  Property,
  PropertyProfile,
  PublicTheme,
  TokenResponse,
  User,
} from "./types";

export const API_BASE =
  process.env.NEXT_PUBLIC_API_URL ?? "http://localhost:8000";

/** Default tenant slug for the public website demo. */
export const DEFAULT_TENANT =
  process.env.NEXT_PUBLIC_DEFAULT_TENANT ?? "northwind";

const ACCESS_KEY = "acre.access";
const REFRESH_KEY = "acre.refresh";

const ACTING_KEY = "acre.acting_tenant";

/**
 * Platform staff have no single tenant, so they "view as" a client by setting an
 * acting tenant slug. It rides along as `X-Tenant` on authenticated requests.
 */
export const actingTenant = {
  get(): string | null {
    return typeof window === "undefined"
      ? null
      : localStorage.getItem(ACTING_KEY);
  },
  set(slug: string) {
    localStorage.setItem(ACTING_KEY, slug);
  },
  clear() {
    localStorage.removeItem(ACTING_KEY);
  },
};

export const tokenStore = {
  get access() {
    return typeof window === "undefined"
      ? null
      : localStorage.getItem(ACCESS_KEY);
  },
  get refresh() {
    return typeof window === "undefined"
      ? null
      : localStorage.getItem(REFRESH_KEY);
  },
  set(tokens: { access_token: string; refresh_token: string }) {
    localStorage.setItem(ACCESS_KEY, tokens.access_token);
    localStorage.setItem(REFRESH_KEY, tokens.refresh_token);
  },
  clear() {
    localStorage.removeItem(ACCESS_KEY);
    localStorage.removeItem(REFRESH_KEY);
  },
};

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string
  ) {
    super(message);
  }
}

interface RequestOpts {
  method?: string;
  body?: unknown;
  tenant?: string;
  auth?: boolean;
}

async function request<T>(path: string, opts: RequestOpts = {}): Promise<T> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  if (opts.tenant) headers["X-Tenant"] = opts.tenant;
  if (opts.auth) {
    const token = tokenStore.access;
    if (token) headers["Authorization"] = `Bearer ${token}`;
    // Staff impersonation: send the acting tenant unless one was set explicitly.
    if (!opts.tenant) {
      const acting = actingTenant.get();
      if (acting) headers["X-Tenant"] = acting;
    }
  }

  const res = await fetch(`${API_BASE}${path}`, {
    method: opts.method ?? "GET",
    headers,
    body: opts.body ? JSON.stringify(opts.body) : undefined,
    cache: "no-store",
  });

  if (!res.ok) {
    let code = "error";
    let message = res.statusText;
    try {
      const data = await res.json();
      code = data?.error?.code ?? code;
      message = data?.error?.message ?? message;
    } catch {
      /* non-JSON error body */
    }
    throw new ApiError(res.status, code, message);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export const api = {
  // ---- public website ----
  publicListings: (tenant = DEFAULT_TENANT) =>
    request<Listing[]>("/public/listings", { tenant }),
  publicListing: (id: string, tenant = DEFAULT_TENANT) =>
    request<Listing>(`/public/listings/${id}`, { tenant }),
  publicTheme: (tenant = DEFAULT_TENANT) =>
    request<PublicTheme>("/public/theme", { tenant }),
  apply: (body: Record<string, unknown>, tenant = DEFAULT_TENANT) =>
    request<ApplyResponse>("/public/applications", {
      method: "POST",
      body,
      tenant,
    }),

  // ---- auth ----
  login: (email: string, password: string) =>
    request<TokenResponse>("/auth/login", {
      method: "POST",
      body: { email, password },
    }),
  me: () => request<User>("/auth/me", { auth: true }),

  // ---- landlord / PM console ----
  portfolioSummary: () =>
    request<PortfolioSummary>("/portfolio/summary", { auth: true }),
  llcGroups: () => request<LlcGroup[]>("/portfolio/llcs", { auth: true }),
  properties: () => request<Property[]>("/properties", { auth: true }),
  property: (id: string) =>
    request<PropertyProfile>(`/properties/${id}`, { auth: true }),
  applications: () => request<Application[]>("/applications", { auth: true }),

  // ---- API tokens ----
  apiTokens: () => request<TokenSummary[]>("/api-tokens", { auth: true }),
  createApiToken: (name: string, scopes: string[]) =>
    request<CreateTokenResponse>("/api-tokens", {
      method: "POST",
      auth: true,
      body: { name, scopes },
    }),
  revokeApiToken: (id: string) =>
    request<void>(`/api-tokens/${id}`, { method: "DELETE", auth: true }),

  // ---- platform (staff) ----
  platformTenants: () =>
    request<TenantSummary[]>("/platform/tenants", { auth: true }),
  platformMetrics: () =>
    request<PlatformMetrics>("/platform/metrics", { auth: true }),

  // ---- modules (tenant software settings) ----
  modules: () => request<ModuleInfo[]>("/modules", { auth: true }),
  setModule: (key: string, enabled: boolean) =>
    request<ModuleInfo>(`/modules/${key}`, {
      method: "PATCH",
      auth: true,
      body: { enabled },
    }),

  // ---- flips module (preview) ----
  flipPipeline: () =>
    request<FlipPipeline>("/modules/flips/pipeline", { auth: true }),
};

export interface TokenSummary {
  id: string;
  name: string;
  prefix: string;
  scopes: string[];
  last_used_at: string | null;
  revoked: boolean;
  created_at: string;
}

export interface CreateTokenResponse extends TokenSummary {
  token: string;
}

export interface TenantSummary {
  id: string;
  slug: string;
  name: string;
  plan: string;
  status: string;
  custom_domain: string | null;
  property_count: number;
  managed_revenue_label: string;
}

export interface PlatformMetrics {
  tenant_count: number;
  active_tenants: number;
  total_properties: number;
  total_managed_revenue_label: string;
}

/** A pluggable module plus its enablement for the active tenant. */
export interface ModuleInfo {
  key: string;
  name: string;
  description: string;
  permissions: string[];
  enabled: boolean;
  default_enabled: boolean;
  preview: boolean;
}

export interface FlipStage {
  key: string;
  label: string;
}

export interface FlipPipeline {
  preview: boolean;
  stages: FlipStage[];
  deals: unknown[];
}
