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
  Workspace,
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
  /**
   * Update ONLY the access token, leaving the refresh token intact. Used by
   * workspace switching, which mints a fresh access token without rotating the
   * refresh token.
   */
  setAccess(token: string) {
    localStorage.setItem(ACCESS_KEY, token);
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
  /** Workspaces the current user can switch between (Acre HQ + tenants). */
  workspaces: () => request<Workspace[]>("/auth/workspaces", { auth: true }),
  /**
   * Switch the active workspace. `null` selects Acre HQ / platform. Returns a
   * fresh access token (refresh token unchanged) plus the updated user.
   */
  switchWorkspace: (tenantId: string | null) =>
    request<SwitchWorkspaceResponse>("/auth/switch", {
      method: "POST",
      auth: true,
      body: { tenant_id: tenantId },
    }),

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

/**
 * IAM (identity & access management) API surface: permissions catalog, profile
 * types, roles, users + their profiles/memberships/roles, and tenant-scoped
 * members. All calls are JWT-authenticated via the shared `request` client.
 */
export const iam = {
  // ---- catalogs ----
  permissions: () =>
    request<PermissionDef[]>("/admin/permissions", { auth: true }),
  profileTypes: () =>
    request<ProfileType[]>("/admin/profile-types", { auth: true }),

  // ---- roles ----
  roles: (params: { scope?: string; tenant_id?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.scope) qs.set("scope", params.scope);
    if (params.tenant_id) qs.set("tenant_id", params.tenant_id);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<Role[]>(`/admin/roles${suffix}`, { auth: true });
  },
  createRole: (body: CreateRoleInput) =>
    request<Role>("/admin/roles", { method: "POST", auth: true, body }),
  updateRole: (id: string, body: UpdateRoleInput) =>
    request<Role>(`/admin/roles/${id}`, { method: "PATCH", auth: true, body }),
  deleteRole: (id: string) =>
    request<{ deleted: true }>(`/admin/roles/${id}`, {
      method: "DELETE",
      auth: true,
    }),

  // ---- users ----
  users: (params: { q?: string; tenant_id?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.q) qs.set("q", params.q);
    if (params.tenant_id) qs.set("tenant_id", params.tenant_id);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<UserSummary[]>(`/admin/users${suffix}`, { auth: true });
  },
  user: (id: string) =>
    request<UserDetail>(`/admin/users/${id}`, { auth: true }),
  createUser: (body: CreateUserInput) =>
    request<UserDetail>("/admin/users", { method: "POST", auth: true, body }),
  updateUser: (id: string, body: UpdateUserInput) =>
    request<UserDetail>(`/admin/users/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),

  // ---- profile + PII ----
  putProfile: (id: string, body: ProfileInput) =>
    request<ProfileDto>(`/admin/users/${id}/profile`, {
      method: "PUT",
      auth: true,
      body,
    }),
  /** Reveal raw PII — only call on an explicit, gated user action. */
  pii: (id: string) =>
    request<UserPii>(`/admin/users/${id}/pii`, { auth: true }),

  // ---- memberships ----
  addMembership: (userId: string, body: MembershipInput) =>
    request<Membership>(`/admin/users/${userId}/memberships`, {
      method: "POST",
      auth: true,
      body,
    }),
  removeMembership: (membershipId: string) =>
    request<void>(`/admin/memberships/${membershipId}`, {
      method: "DELETE",
      auth: true,
    }),

  // ---- user roles ----
  assignRole: (userId: string, body: { role_id: string; tenant_id?: string }) =>
    request<void>(`/admin/users/${userId}/roles`, {
      method: "POST",
      auth: true,
      body,
    }),
  revokeRole: (userRoleId: string) =>
    request<void>(`/admin/user-roles/${userRoleId}`, {
      method: "DELETE",
      auth: true,
    }),

  // ---- audit log ----
  audit: (params: { limit?: number; action?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.limit != null) qs.set("limit", String(params.limit));
    if (params.action) qs.set("action", params.action);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<AuditEntry[]>(`/admin/audit${suffix}`, { auth: true });
  },

  // ---- tenant-scoped members (client admin) ----
  members: () => request<Member[]>("/members", { auth: true }),
  inviteMember: (body: InviteMemberInput) =>
    request<Member>("/members", { method: "POST", auth: true, body }),
};

/**
 * Response from `POST /auth/switch`. Unlike login, this returns only a fresh
 * access token (the refresh token is unchanged) plus the updated user.
 */
export interface SwitchWorkspaceResponse {
  access_token: string;
  token_type: string;
  expires_in: number;
  user: User;
}

/** A single audit-log entry from `GET /admin/audit`. */
export interface AuditEntry {
  id: string;
  actor_user_id: string | null;
  actor_name: string | null;
  action: string;
  target_type: string | null;
  target_id: string | null;
  tenant_id: string | null;
  metadata: unknown | null;
  created_at: string;
}

// ---- IAM types ---------------------------------------------------------------

/** A single grantable permission in the catalog. */
export interface PermissionDef {
  key: string;
  category: string;
  label: string;
  description: string;
  scope: string;
}

/** A persona/profile-type a membership can take. */
export interface ProfileType {
  key: string;
  scope: string;
  label: string;
  description: string;
  default_role: string;
}

/** A role: a named bundle of permissions, scoped platform- or tenant-wide. */
export interface Role {
  id: string;
  scope: string;
  tenant_id: string | null;
  key: string;
  name: string;
  description: string;
  is_system: boolean;
  permissions: string[];
}

export interface CreateRoleInput {
  scope: string;
  tenant_id?: string;
  key: string;
  name: string;
  description: string;
  permissions: string[];
}

export interface UpdateRoleInput {
  name?: string;
  description?: string;
  permissions?: string[];
}

/** A row in the user directory. */
export interface UserSummary {
  id: string;
  email: string;
  username: string | null;
  name: string;
  status: string;
  is_platform_staff: boolean;
  tenant_id: string | null;
}

/** Masked profile fields (safe to render); raw PII comes from `iam.pii`. */
export interface ProfileDto {
  legal_first_name: string | null;
  legal_middle_name: string | null;
  legal_last_name: string | null;
  preferred_name: string | null;
  date_of_birth: string | null;
  phone: string | null;
  address_line1: string | null;
  address_line2: string | null;
  city: string | null;
  region: string | null;
  postal_code: string | null;
  country: string | null;
  ssn_last4: string | null;
  gov_id_type: string | null;
  gov_id_last4: string | null;
  photo_url: string | null;
  has_ssn: boolean;
  has_gov_id: boolean;
}

/** Editable profile payload (PUT). Sensitive fields are write-only. */
export interface ProfileInput {
  legal_first_name?: string;
  legal_middle_name?: string;
  legal_last_name?: string;
  preferred_name?: string;
  /** "YYYY-MM-DD" */
  date_of_birth?: string;
  phone?: string;
  address_line1?: string;
  address_line2?: string;
  city?: string;
  region?: string;
  postal_code?: string;
  country?: string;
  ssn?: string;
  gov_id_number?: string;
  gov_id_type?: string;
}

/** A user's membership in a scope/tenant under a given persona. */
export interface Membership {
  id: string;
  scope: string;
  tenant_id: string | null;
  profile_type: string;
  title: string | null;
  status: string;
  is_primary: boolean;
}

export interface MembershipInput {
  scope: string;
  tenant_id?: string;
  profile_type: string;
  title?: string;
}

/** A role assignment on a user. */
export interface UserRole {
  id: string;
  role_id: string;
  role_key: string;
  role_name: string;
  tenant_id: string | null;
}

/** Full user record returned by detail / mutation endpoints. */
export interface UserDetail {
  id: string;
  email: string;
  username: string | null;
  name: string;
  status: string;
  is_platform_staff: boolean;
  tenant_id: string | null;
  profile: ProfileDto | null;
  memberships: Membership[];
  roles: UserRole[];
}

export interface CreateUserInput {
  email: string;
  username?: string;
  name: string;
  password?: string;
  membership?: MembershipInput;
  profile?: ProfileInput;
}

export interface UpdateUserInput {
  name?: string;
  username?: string;
  status?: string;
}

/** Raw, sensitive PII — never persist to long-lived state. */
export interface UserPii {
  ssn?: string;
  gov_id_number?: string;
}

/** A tenant-scoped member row (client-admin view). */
export interface Member {
  membership_id: string;
  user_id: string;
  name: string;
  email: string;
  profile_type: string;
  title: string | null;
  status: string;
}

export interface InviteMemberInput {
  email: string;
  name: string;
  profile_type: string;
  title?: string;
}

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
