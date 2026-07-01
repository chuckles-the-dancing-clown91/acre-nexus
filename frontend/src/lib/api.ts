// Typed API client for the Acre Rust backend.
//
// Multi-tenancy: public requests carry the tenant via the `X-Tenant` header
// (slug). Authenticated requests carry a JWT `Authorization: Bearer` token;
// platform staff can additionally pass `X-Tenant` to "view as" a client.

import type {
  Application,
  ApplicationWorkflow,
  AppWorkflowCatalog,
  ApplyResponse,
  Assignment,
  CreateAssignmentInput,
  Counterparty,
  SettingView,
  CounterpartyDetail,
  CounterpartyNote,
  CreateCounterpartyInput,
  CreateLeaseInput,
  CreateLienInput,
  CreateMortgageInput,
  CreateOwnershipInput,
  CreateTicketInput,
  CreateUnitInput,
  EnrichmentRun,
  EnrichResponse,
  Lease,
  LeaseDetail,
  LeasePayment,
  Lien,
  Listing,
  LlcGroup,
  MaintenanceTicket,
  Mortgage,
  OnboardInput,
  OnboardResponse,
  Ownership,
  PortfolioSummary,
  Property,
  PropertyIntel,
  PropertyProfile,
  PublicTheme,
  RecordPaymentInput,
  TicketDetail,
  TokenResponse,
  Unit,
  UpdateTicketInput,
  User,
  Workflow,
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
  // ---- property intelligence (enrichment) ----
  propertyIntel: (id: string) =>
    request<PropertyIntel>(`/properties/${id}/intel`, { auth: true }),
  enrichProperty: (id: string, sources: string[] = []) =>
    request<EnrichResponse>(`/properties/${id}/enrich`, {
      method: "POST",
      auth: true,
      body: { sources },
    }),
  propertyEnrichment: (id: string) =>
    request<EnrichmentRun[]>(`/properties/${id}/enrichment`, { auth: true }),
  // ---- onboarding ----
  onboardProperty: (body: OnboardInput) =>
    request<OnboardResponse>("/properties/onboard", {
      method: "POST",
      auth: true,
      body,
    }),
  // ---- staff assignments (property + LLC) ----
  propertyAssignments: (propertyId: string) =>
    request<Assignment[]>(`/properties/${propertyId}/assignments`, {
      auth: true,
    }),
  createPropertyAssignment: (propertyId: string, body: CreateAssignmentInput) =>
    request<Assignment>(`/properties/${propertyId}/assignments`, {
      method: "POST",
      auth: true,
      body,
    }),
  deletePropertyAssignment: (propertyId: string, assignmentId: string) =>
    request<{ removed: boolean }>(
      `/properties/${propertyId}/assignments/${assignmentId}`,
      { method: "DELETE", auth: true }
    ),
  entityAssignments: (entityId: string) =>
    request<Assignment[]>(`/entities/${entityId}/assignments`, { auth: true }),
  createEntityAssignment: (entityId: string, body: CreateAssignmentInput) =>
    request<Assignment>(`/entities/${entityId}/assignments`, {
      method: "POST",
      auth: true,
      body,
    }),
  deleteEntityAssignment: (entityId: string, assignmentId: string) =>
    request<{ removed: boolean }>(
      `/entities/${entityId}/assignments/${assignmentId}`,
      { method: "DELETE", auth: true }
    ),
  // ---- financing (mortgages) ----
  mortgages: (propertyId: string) =>
    request<Mortgage[]>(`/properties/${propertyId}/mortgages`, { auth: true }),
  createMortgage: (propertyId: string, body: CreateMortgageInput) =>
    request<Mortgage>(`/properties/${propertyId}/mortgages`, {
      method: "POST",
      auth: true,
      body,
    }),
  deleteMortgage: (id: string) =>
    request<void>(`/mortgages/${id}`, { method: "DELETE", auth: true }),
  // ---- investment workflow ----
  workflow: (propertyId: string) =>
    request<Workflow>(`/properties/${propertyId}/workflow`, { auth: true }),
  advanceWorkflow: (propertyId: string, to_stage: string, note?: string) =>
    request<Workflow>(`/properties/${propertyId}/workflow/advance`, {
      method: "POST",
      auth: true,
      body: { to_stage, note },
    }),
  /** The strategy + stage templates, independent of any property (board columns). */
  workflowCatalog: () =>
    request<WorkflowStrategy[]>("/workflows/catalog", { auth: true }),
  // ---- entities registry (counterparties) ----
  entities: (kind?: string) =>
    request<Counterparty[]>(
      `/entities${kind ? `?kind=${encodeURIComponent(kind)}` : ""}`,
      { auth: true }
    ),
  entity: (id: string) =>
    request<CounterpartyDetail>(`/entities/${id}`, { auth: true }),
  createEntity: (body: CreateCounterpartyInput) =>
    request<Counterparty>("/entities", { method: "POST", auth: true, body }),
  addEntityNote: (id: string, body: string) =>
    request<CounterpartyNote>(`/entities/${id}/notes`, {
      method: "POST",
      auth: true,
      body: { body },
    }),
  // ---- rentals: units ----
  units: (propertyId: string) =>
    request<Unit[]>(`/properties/${propertyId}/units`, { auth: true }),
  createUnit: (propertyId: string, body: CreateUnitInput) =>
    request<Unit>(`/properties/${propertyId}/units`, {
      method: "POST",
      auth: true,
      body,
    }),
  // ---- rentals: leases ----
  leases: (params: { status?: string; property_id?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.status) qs.set("status", params.status);
    if (params.property_id) qs.set("property_id", params.property_id);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<Lease[]>(`/leases${suffix}`, { auth: true });
  },
  propertyLeases: (propertyId: string) =>
    request<Lease[]>(`/properties/${propertyId}/leases`, { auth: true }),
  createLease: (propertyId: string, body: CreateLeaseInput) =>
    request<Lease>(`/properties/${propertyId}/leases`, {
      method: "POST",
      auth: true,
      body,
    }),
  lease: (id: string) => request<LeaseDetail>(`/leases/${id}`, { auth: true }),
  recordPayment: (leaseId: string, body: RecordPaymentInput) =>
    request<LeasePayment>(`/leases/${leaseId}/payments`, {
      method: "POST",
      auth: true,
      body,
    }),
  // ---- maintenance: tickets ----
  tickets: (
    params: { status?: string; property_id?: string; priority?: string } = {}
  ) => {
    const qs = new URLSearchParams();
    if (params.status) qs.set("status", params.status);
    if (params.property_id) qs.set("property_id", params.property_id);
    if (params.priority) qs.set("priority", params.priority);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<MaintenanceTicket[]>(`/tickets${suffix}`, { auth: true });
  },
  propertyTickets: (propertyId: string) =>
    request<MaintenanceTicket[]>(`/properties/${propertyId}/tickets`, {
      auth: true,
    }),
  createTicket: (propertyId: string, body: CreateTicketInput) =>
    request<MaintenanceTicket>(`/properties/${propertyId}/tickets`, {
      method: "POST",
      auth: true,
      body,
    }),
  ticket: (id: string) =>
    request<TicketDetail>(`/tickets/${id}`, { auth: true }),
  updateTicket: (id: string, body: UpdateTicketInput) =>
    request<MaintenanceTicket>(`/tickets/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  addTicketComment: (id: string, body: string) =>
    request<unknown>(`/tickets/${id}/comments`, {
      method: "POST",
      auth: true,
      body: { body },
    }),
  // ---- title: ownership + liens ----
  ownership: (propertyId: string) =>
    request<Ownership[]>(`/properties/${propertyId}/ownership`, { auth: true }),
  createOwnership: (propertyId: string, body: CreateOwnershipInput) =>
    request<Ownership>(`/properties/${propertyId}/ownership`, {
      method: "POST",
      auth: true,
      body,
    }),
  liens: (propertyId: string) =>
    request<Lien[]>(`/properties/${propertyId}/liens`, { auth: true }),
  createLien: (propertyId: string, body: CreateLienInput) =>
    request<Lien>(`/properties/${propertyId}/liens`, {
      method: "POST",
      auth: true,
      body,
    }),
  applications: () => request<Application[]>("/applications", { auth: true }),
  updateApplication: (id: string, status: string) =>
    request<Application>(`/applications/${id}`, {
      method: "PATCH",
      auth: true,
      body: { status },
    }),

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

  // ---- leasing lifecycle: fees, vehicles, charges, documents, history ----
  fees: () => request<Fee[]>("/fees", { auth: true }),
  createFee: (body: CreateFeeInput) =>
    request<Fee>("/fees", { method: "POST", auth: true, body }),
  updateFee: (id: string, body: UpdateFeeInput) =>
    request<Fee>(`/fees/${id}`, { method: "PATCH", auth: true, body }),
  deleteFee: (id: string) =>
    request<void>(`/fees/${id}`, { method: "DELETE", auth: true }),

  vehicles: (params: { lease_id?: string; application_id?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.lease_id) qs.set("lease_id", params.lease_id);
    if (params.application_id) qs.set("application_id", params.application_id);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<VehicleProfile[]>(`/vehicles${suffix}`, { auth: true });
  },
  createVehicle: (body: CreateVehicleInput) =>
    request<VehicleProfile>("/vehicles", { method: "POST", auth: true, body }),
  deleteVehicle: (id: string) =>
    request<void>(`/vehicles/${id}`, { method: "DELETE", auth: true }),

  leaseCharges: (leaseId: string) =>
    request<ChargesResp>(`/leases/${leaseId}/charges`, { auth: true }),
  addLeaseCharge: (leaseId: string, body: AddChargeInput) =>
    request<LeaseChargeDto>(`/leases/${leaseId}/charges`, {
      method: "POST",
      auth: true,
      body,
    }),
  deleteLeaseCharge: (id: string) =>
    request<void>(`/lease-charges/${id}`, { method: "DELETE", auth: true }),
  applyFees: (leaseId: string) =>
    request<ApplyFeesResp>(`/leases/${leaseId}/apply-fees`, {
      method: "POST",
      auth: true,
    }),

  generateLeaseDoc: (leaseId: string) =>
    request<LeaseDocDto>(`/leases/${leaseId}/document/generate`, {
      method: "POST",
      auth: true,
    }),
  leaseDoc: (leaseId: string) =>
    request<LeaseDocDto>(`/leases/${leaseId}/document`, { auth: true }),
  signLeaseDoc: (leaseId: string, signed_by: string) =>
    request<LeaseDocDto>(`/leases/${leaseId}/document/sign`, {
      method: "POST",
      auth: true,
      body: { signed_by },
    }),

  convertApplication: (applicationId: string, body: ConvertInput) =>
    request<Lease>(`/applications/${applicationId}/convert-to-lease`, {
      method: "POST",
      auth: true,
      body,
    }),

  // ---- application workflow ----
  applicationWorkflowCatalog: () =>
    request<AppWorkflowCatalog>("/applications/workflow/catalog", {
      auth: true,
    }),
  applicationWorkflow: (id: string) =>
    request<ApplicationWorkflow>(`/applications/${id}/workflow`, {
      auth: true,
    }),
  advanceApplication: (id: string, to_status: string, note?: string) =>
    request<Application>(`/applications/${id}/advance`, {
      method: "POST",
      auth: true,
      body: { to_status, ...(note ? { note } : {}) },
    }),

  // ---- application reuse (gated by the application_reuse setting) ----
  reusableApplications: (email: string) =>
    request<Application[]>(
      `/applications/reusable?email=${encodeURIComponent(email)}`,
      { auth: true }
    ),
  reuseApplication: (source_application_id: string, listing_id?: string) =>
    request<Application>("/applications/reuse", {
      method: "POST",
      auth: true,
      body: { source_application_id, ...(listing_id ? { listing_id } : {}) },
    }),

  // ---- system settings ----
  settings: () => request<SettingView[]>("/settings", { auth: true }),
  setSetting: (key: string, value: unknown) =>
    request<SettingView>(`/settings/${key}`, {
      method: "PUT",
      auth: true,
      body: { value },
    }),

  tenantHistory: () =>
    request<TenantHistoryRow[]>("/tenant-history", { auth: true }),
  propertyTenantHistory: (propertyId: string) =>
    request<TenantHistoryRow[]>(`/properties/${propertyId}/tenant-history`, {
      auth: true,
    }),

  // ---- branding / theme ----
  theme: () => request<ThemeConfig>("/theme", { auth: true }),
  updateTheme: (body: UpdateThemeInput) =>
    request<ThemeConfig>("/theme", { method: "PUT", auth: true, body }),

  // ---- legal entities (LLCs) ----
  legalEntities: () => request<LegalEntity[]>("/llcs", { auth: true }),

  // ---- white-label domains & routing ----
  domains: () => request<DomainInfo[]>("/domains", { auth: true }),
  createDomain: (hostname: string, audience: string) =>
    request<DomainInfo>("/domains", {
      method: "POST",
      auth: true,
      body: { hostname, audience },
    }),
  verifyDomain: (id: string) =>
    request<DomainInfo>(`/domains/${id}/verify`, {
      method: "POST",
      auth: true,
    }),
  deleteDomain: (id: string) =>
    request<void>(`/domains/${id}`, { method: "DELETE", auth: true }),
  /** Public: resolve a host to its tenant + audience + branding (no auth). */
  resolveHost: (host: string) =>
    request<ResolveResult>(`/public/resolve?host=${encodeURIComponent(host)}`),

  // ---- onboarding workflow (per-tenant setup state machine) ----
  onboardingWorkflow: () =>
    request<OnboardingSnapshot>("/onboarding/workflow", { auth: true }),
  advanceOnboarding: () =>
    request<OnboardingSnapshot>("/onboarding/workflow/advance", {
      method: "POST",
      auth: true,
    }),

  // ---- portfolios ----
  portfolios: () => request<PortfolioInfo[]>("/portfolios", { auth: true }),
  createPortfolio: (name: string, strategy?: string) =>
    request<PortfolioInfo>("/portfolios", {
      method: "POST",
      auth: true,
      body: { name, strategy },
    }),

  // ---- legal-entity cap table + banking ----
  capTable: (entityId: string) =>
    request<CapTable>(`/entities/${entityId}/cap-table`, { auth: true }),
  addOwnership: (entityId: string, body: AddOwnershipInput) =>
    request<unknown>(`/entities/${entityId}/cap-table`, {
      method: "POST",
      auth: true,
      body,
    }),
  bankAccounts: (entityId: string) =>
    request<BankAccount[]>(`/entities/${entityId}/bank-accounts`, {
      auth: true,
    }),
  createBankAccount: (entityId: string, body: CreateBankAccountInput) =>
    request<BankAccount>(`/entities/${entityId}/bank-accounts`, {
      method: "POST",
      auth: true,
      body,
    }),

  // ---- platform plane: staff + audited impersonation + provisioning ----
  platformStaff: () =>
    request<PlatformStaff[]>("/platform/staff", { auth: true }),
  impersonations: () =>
    request<ImpersonationSummary[]>("/platform/impersonations", { auth: true }),
  impersonate: (tenant: string, reason: string) =>
    request<ImpersonationResult>("/platform/impersonate", {
      method: "POST",
      auth: true,
      body: { tenant, reason },
    }),
  revokeImpersonation: (id: string) =>
    request<void>(`/platform/impersonations/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  provisionTenant: (body: ProvisionInput) =>
    request<ProvisionResult>("/platform/provision", {
      method: "POST",
      auth: true,
      body,
    }),
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
  assignRole: (
    userId: string,
    body: {
      role_id: string;
      tenant_id?: string;
      scope?: string;
      scope_ref_id?: string;
    }
  ) =>
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
  /** Kind of principal: "user" | "api_token" | "public" | "system". */
  principal_kind: string | null;
  // Request context — present on per-request ("http.request") entries.
  method: string | null;
  path: string | null;
  status_code: number | null;
  ip: string | null;
  duration_ms: number | null;
  request_id: string | null;
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
  /** Coverage scope: platform | tenant | entity | portfolio | property. */
  scope: string;
  scope_ref_id: string | null;
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

// ---- leasing lifecycle ----

export interface Fee {
  id: string;
  code: string;
  kind: string;
  label: string;
  amount_cents: number;
  amount_label: string;
  recurring: boolean;
  condition_type: string;
  verbiage: string | null;
  active: boolean;
}

export interface CreateFeeInput {
  code: string;
  kind: string;
  label: string;
  amount_cents: number;
  recurring?: boolean;
  condition_type?: string;
  verbiage?: string;
}

export interface UpdateFeeInput {
  label?: string;
  amount_cents?: number;
  recurring?: boolean;
  condition_type?: string;
  verbiage?: string;
  active?: boolean;
}

export interface VehicleProfile {
  id: string;
  lease_id: string | null;
  application_id: string | null;
  user_id: string | null;
  make: string;
  model: string;
  year: number | null;
  color: string | null;
  license_plate: string | null;
  plate_state: string | null;
  notes: string | null;
  label: string;
}

export interface CreateVehicleInput {
  lease_id?: string;
  application_id?: string;
  make: string;
  model: string;
  year?: number;
  color?: string;
  license_plate?: string;
  plate_state?: string;
  notes?: string;
}

export interface LeaseChargeDto {
  id: string;
  lease_id: string;
  kind: string;
  code: string | null;
  label: string;
  amount_cents: number;
  amount_label: string;
  recurring: boolean;
  source: string;
  verbiage: string | null;
}

export interface ChargesResp {
  charges: LeaseChargeDto[];
  base_rent_cents: number;
  base_rent_label: string;
  monthly_total_cents: number;
  monthly_total_label: string;
}

export interface AddChargeInput {
  kind: string;
  code?: string;
  label: string;
  amount_cents: number;
  recurring?: boolean;
  verbiage?: string;
}

export interface ApplyFeesResp {
  applied: number;
  charges: LeaseChargeDto[];
}

export interface LeaseDocDto {
  id: string;
  lease_id: string;
  title: string;
  body: string;
  format: string;
  status: string;
  generated_at: string;
  signed_at: string | null;
  signed_by: string | null;
  signed_hash: string | null;
}

export interface ConvertInput {
  property_id: string;
  unit_id?: string;
  rent_cents: number;
  deposit_cents?: number;
  start_date?: string;
  end_date?: string;
}

export interface TenancySummary {
  lease_id: string;
  property_id: string;
  property_name: string | null;
  unit_id: string | null;
  status: string;
  payment_status: string;
  start_date: string;
  end_date: string | null;
  rent_cents: number;
  rent_label: string;
  balance_cents: number;
  balance_label: string;
  from_application: boolean;
}

export interface TenantHistoryRow {
  tenant_name: string;
  tenant_email: string | null;
  tenant_phone: string | null;
  current: boolean;
  lease_count: number;
  latest_start: string;
  tenancies: TenancySummary[];
}

/** A tenant's white-label branding configuration. */
export interface ThemeConfig {
  company_name: string;
  logo_url: string | null;
  primary_color: string;
  accent_color: string;
  default_mode: string;
  legal_templates: unknown;
}

export interface UpdateThemeInput {
  company_name?: string;
  logo_url?: string;
  primary_color?: string;
  accent_color?: string;
  default_mode?: string;
}

/** A legal entity (LLC/LP/…) — the spec's enriched holding entity. */
export interface LegalEntity {
  id: string;
  name: string;
  ein: string;
  state: string;
  entity_type: string;
  registered_agent: string | null;
  status: string;
}

/** A stage in a strategy's workflow template. */
export interface WorkflowCatalogStage {
  key: string;
  label: string;
}

/** An investment strategy + its ordered stage template (board columns). */
export interface WorkflowStrategy {
  key: string;
  label: string;
  description: string;
  stages: WorkflowCatalogStage[];
}

// ---- tenancy spec: domains, onboarding, multi-entity, platform plane ----

export interface DnsInstructions {
  cname_target: string;
  txt_name: string;
  txt_value: string;
}

export interface DomainInfo {
  id: string;
  hostname: string;
  kind: string;
  audience: string;
  verification_token: string | null;
  verified: boolean;
  verified_at: string | null;
  tls_status: string;
  dns_instructions: DnsInstructions | null;
}

export interface ResolveResult {
  tenant_id: string;
  tenant_slug: string;
  audience: string;
  company_name: string;
  primary_color: string;
  accent_color: string;
}

export interface OnboardingStep {
  key: string;
  label: string;
  complete: boolean;
  optional: boolean;
}

export interface OnboardingSnapshot {
  state: string;
  steps: OnboardingStep[];
  live: boolean;
}

export interface PortfolioInfo {
  id: string;
  name: string;
  strategy: string;
  property_count: number;
}

export interface CapTableRow {
  ownership_id: string;
  owner_id: string;
  owner_name: string;
  owner_kind: string;
  ownership_bps: number;
  ownership_label: string;
  role: string;
}

export interface CapTable {
  entity_id: string;
  rows: CapTableRow[];
  total_bps: number;
  total_label: string;
}

export interface AddOwnershipInput {
  owner_id?: string;
  owner_name?: string;
  owner_kind?: string;
  ownership_bps: number;
  role?: string;
}

export interface BankAccount {
  id: string;
  entity_id: string;
  kind: string;
  institution: string;
  masked_number: string | null;
  status: string;
}

export interface CreateBankAccountInput {
  kind: string;
  institution: string;
  account_number?: string;
}

export interface PlatformStaff {
  id: string;
  user_id: string;
  email: string;
  name: string;
  status: string;
}

export interface ImpersonationSummary {
  id: string;
  platform_staff_id: string;
  tenant_id: string;
  tenant_name: string | null;
  reason: string;
  expires_at: string;
  revoked_at: string | null;
  active: boolean;
  created_at: string;
}

export interface ImpersonationResult {
  session_id: string;
  tenant_id: string;
  reason: string;
  expires_at: string;
  access_token: string;
  token_type: string;
  expires_in: number;
}

export interface ProvisionInput {
  slug: string;
  name: string;
  plan?: string;
  owner_email: string;
  owner_name?: string;
  owner_password?: string;
}

export interface ProvisionResult {
  tenant_id: string;
  slug: string;
  subdomain: string;
  owner_user_id: string;
  owner_email: string;
  temp_password: string | null;
}
