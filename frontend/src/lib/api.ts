// Typed API client for the Acre Rust backend.
//
// Multi-tenancy: public requests carry the tenant via the `X-Tenant` header
// (slug). Authenticated requests carry a JWT `Authorization: Bearer` token;
// platform staff can additionally pass `X-Tenant` to "view as" a client.

import type {
  Application,
  Asset,
  InventoryItem,
  TicketLine,
  CreateAssetInput,
  UpdateAssetInput,
  ApplicationWorkflow,
  ConsoleListing,
  CreateApplicationInput,
  CreateListingInput,
  PortalApplyInput,
  UpdateListingInput,
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
  MaintenancePlan,
  MaintenanceTicket,
  Mortgage,
  OnboardInput,
  OnboardResponse,
  Ownership,
  PortfolioSummary,
  Property,
  PropertyIntel,
  PropertyProfile,
  PropertyFinancials,
  PropertyMaintenance,
  PublicTheme,
  RecordPaymentInput,
  ScreeningReport,
  TicketComment,
  TicketDetail,
  TicketQuote,
  TokenResponse,
  Unit,
  UpdateTicketInput,
  User,
  Workflow,
  Workspace,
} from "./types";
import { logError } from "./log";

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

  let res: Response;
  try {
    res = await fetch(`${API_BASE}${path}`, {
      method: opts.method ?? "GET",
      headers,
      body: opts.body ? JSON.stringify(opts.body) : undefined,
      cache: "no-store",
    });
  } catch (e) {
    // A network-level failure (offline, DNS, CORS, connection refused) has no
    // legitimate "expected" case anywhere it's called from, unlike a 4xx/5xx
    // response — log it centrally here so it's visible even when a caller's
    // own `.catch` doesn't, then re-throw for the caller to handle as usual.
    logError(`${opts.method ?? "GET"} ${path} failed`, e);
    throw e;
  }

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
  // ---- property profile tabs (aggregations) ----
  propertyFinancials: (id: string) =>
    request<PropertyFinancials>(`/properties/${id}/financials`, { auth: true }),
  propertyMaintenance: (id: string) =>
    request<PropertyMaintenance>(`/properties/${id}/maintenance`, {
      auth: true,
    }),
  propertyDocuments: (id: string) =>
    request<PropertyDocuments>(`/properties/${id}/documents`, { auth: true }),
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
  addTicketComment: (
    id: string,
    body: string,
    visibility: "public" | "internal" = "public"
  ) =>
    request<unknown>(`/tickets/${id}/comments`, {
      method: "POST",
      auth: true,
      body: { body, visibility },
    }),
  // ---- ticket lines (parts / labor / fees) + inventory ----
  addTicketLine: (
    ticketId: string,
    body: {
      kind?: string;
      description?: string;
      inventory_item_id?: string;
      serial_number?: string;
      quantity?: number;
      unit_cost_cents?: number;
    }
  ) =>
    request<TicketLine>(`/tickets/${ticketId}/lines`, {
      method: "POST",
      auth: true,
      body,
    }),
  removeTicketLine: (id: string) =>
    request<{ deleted: boolean }>(`/ticket-lines/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  inventory: (
    params: { property_id?: string; status?: string; low_stock?: boolean } = {}
  ) => {
    const qs = new URLSearchParams();
    if (params.property_id) qs.set("property_id", params.property_id);
    if (params.status) qs.set("status", params.status);
    if (params.low_stock) qs.set("low_stock", "true");
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<InventoryItem[]>(`/inventory${suffix}`, { auth: true });
  },
  createInventory: (body: {
    property_id?: string;
    name: string;
    sku?: string;
    category?: string;
    quantity?: number;
    unit_cost_cents?: number;
    reorder_level?: number;
    storage_location?: string;
    serial_numbers?: string[];
    notes?: string;
  }) =>
    request<InventoryItem>("/inventory", { method: "POST", auth: true, body }),
  updateInventory: (
    id: string,
    body: {
      name?: string;
      sku?: string;
      category?: string;
      quantity?: number;
      unit_cost_cents?: number;
      reorder_level?: number;
      storage_location?: string;
      serial_numbers?: string[];
      notes?: string;
      status?: "active" | "archived";
    }
  ) =>
    request<InventoryItem>(`/inventory/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  // ---- equipment registry (assets) ----
  assets: (
    params: { property_id?: string; unit_id?: string; status?: string } = {}
  ) => {
    const qs = new URLSearchParams();
    if (params.property_id) qs.set("property_id", params.property_id);
    if (params.unit_id) qs.set("unit_id", params.unit_id);
    if (params.status) qs.set("status", params.status);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<Asset[]>(`/assets${suffix}`, { auth: true });
  },
  createAsset: (body: CreateAssetInput) =>
    request<Asset>("/assets", { method: "POST", auth: true, body }),
  updateAsset: (id: string, body: UpdateAssetInput) =>
    request<Asset>(`/assets/${id}`, { method: "PATCH", auth: true, body }),
  // ---- helpdesk (Phase 6): quotes + preventive plans ----
  addTicketQuote: (
    ticketId: string,
    body: { entity_id?: string; description: string; amount_cents: number }
  ) =>
    request<TicketQuote>(`/tickets/${ticketId}/quotes`, {
      method: "POST",
      auth: true,
      body,
    }),
  approveTicketQuote: (id: string) =>
    request<TicketQuote>(`/ticket-quotes/${id}/approve`, {
      method: "POST",
      auth: true,
      body: {},
    }),
  rejectTicketQuote: (id: string) =>
    request<TicketQuote>(`/ticket-quotes/${id}/reject`, {
      method: "POST",
      auth: true,
      body: {},
    }),
  maintenancePlans: () =>
    request<MaintenancePlan[]>("/maintenance-plans", { auth: true }),
  createMaintenancePlan: (body: {
    property_id: string;
    unit_id?: string;
    title: string;
    description?: string;
    category?: string;
    priority?: string;
    cadence_days: number;
    next_due_date: string;
  }) =>
    request<MaintenancePlan>("/maintenance-plans", {
      method: "POST",
      auth: true,
      body,
    }),
  updateMaintenancePlan: (
    id: string,
    body: {
      title?: string;
      description?: string;
      category?: string;
      priority?: string;
      cadence_days?: number;
      next_due_date?: string;
      active?: boolean;
    }
  ) =>
    request<MaintenancePlan>(`/maintenance-plans/${id}`, {
      method: "PATCH",
      auth: true,
      body,
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
  /** Back-office intake: staff enter an application on an applicant's behalf. */
  createApplication: (body: CreateApplicationInput) =>
    request<Application>("/applications", { method: "POST", auth: true, body }),
  updateApplication: (id: string, status: string) =>
    request<Application>(`/applications/${id}`, {
      method: "PATCH",
      auth: true,
      body: { status },
    }),
  /** The application's screening (consumer) report — requires screening:read. */
  screeningReport: (applicationId: string) =>
    request<ScreeningReport>(`/applications/${applicationId}/screening`, {
      auth: true,
    }),
  /** Send + file the FCRA adverse-action notice for a declined application. */
  sendAdverseAction: (applicationId: string) =>
    request<Application>(`/applications/${applicationId}/adverse-action`, {
      method: "POST",
      auth: true,
    }),

  // ---- renter portal: apply + track as the signed-in user ----
  myApplications: () =>
    request<Application[]>("/my/applications", { auth: true }),
  myApply: (body: PortalApplyInput) =>
    request<Application>("/my/applications", {
      method: "POST",
      auth: true,
      body,
    }),

  // ---- self-service profile + vehicles (white-glove source of truth) ----
  myProfile: () => request<MyProfileView>("/my/profile", { auth: true }),
  updateMyProfile: (body: ProfileInput) =>
    request<MyProfileView>("/my/profile", { method: "PUT", auth: true, body }),
  myVehicles: () => request<VehicleProfile[]>("/my/vehicles", { auth: true }),
  addMyVehicle: (body: CreateVehicleInput) =>
    request<VehicleProfile>("/my/vehicles", {
      method: "POST",
      auth: true,
      body,
    }),
  deleteMyVehicle: (id: string) =>
    request<{ deleted: boolean }>(`/my/vehicles/${id}`, {
      method: "DELETE",
      auth: true,
    }),

  // ---- console listing management ----
  consoleListings: (params: { property_id?: string; status?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.property_id) qs.set("property_id", params.property_id);
    if (params.status) qs.set("status", params.status);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<ConsoleListing[]>(`/listings${suffix}`, { auth: true });
  },
  createListing: (propertyId: string, body: CreateListingInput) =>
    request<ConsoleListing>(`/properties/${propertyId}/listings`, {
      method: "POST",
      auth: true,
      body,
    }),
  updateListing: (id: string, body: UpdateListingInput) =>
    request<ConsoleListing>(`/listings/${id}`, {
      method: "PATCH",
      auth: true,
      body,
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

  // ---- SaaS billing: platform plane (staff) ----
  platformBillingOverview: () =>
    request<BillingOverview>("/platform/billing/overview", { auth: true }),
  platformBillingInvoices: (
    params: { status?: string; period?: string } = {}
  ) => {
    const qs = new URLSearchParams();
    if (params.status) qs.set("status", params.status);
    if (params.period) qs.set("period", params.period);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<PlatformInvoice[]>(`/platform/billing/invoices${suffix}`, {
      auth: true,
    });
  },
  platformBillingRun: (period?: string) =>
    request<{ period: string; generated: number }>("/platform/billing/run", {
      method: "POST",
      auth: true,
      body: { period },
    }),
  platformInvoicePay: (id: string) =>
    request<PlatformInvoice>(`/platform/billing/invoices/${id}/pay`, {
      method: "POST",
      auth: true,
    }),
  platformInvoiceVoid: (id: string) =>
    request<PlatformInvoice>(`/platform/billing/invoices/${id}/void`, {
      method: "POST",
      auth: true,
    }),
  platformSetPlan: (tenantId: string, plan: string) =>
    request<{ tenant_id: string; plan: string }>(
      `/platform/billing/tenants/${tenantId}/plan`,
      { method: "PATCH", auth: true, body: { plan } }
    ),

  // ---- modules (tenant software settings) ----
  modules: () => request<ModuleInfo[]>("/modules", { auth: true }),
  setModule: (key: string, enabled: boolean) =>
    request<ModuleInfo>(`/modules/${key}`, {
      method: "PATCH",
      auth: true,
      body: { enabled },
    }),

  // ---- flips module: acquisition deal pipeline + underwriting ----
  flipPipeline: () =>
    request<FlipPipeline>("/modules/flips/pipeline", { auth: true }),
  flipDeals: (params: { stage?: string; strategy?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.stage) qs.set("stage", params.stage);
    if (params.strategy) qs.set("strategy", params.strategy);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<FlipDeal[]>(`/modules/flips/deals${suffix}`, { auth: true });
  },
  flipDeal: (id: string) =>
    request<DealDetail>(`/modules/flips/deals/${id}`, { auth: true }),
  createFlipDeal: (body: CreateDealInput) =>
    request<FlipDeal>("/modules/flips/deals", {
      method: "POST",
      auth: true,
      body,
    }),
  updateFlipDeal: (id: string, body: UpdateDealInput) =>
    request<FlipDeal>(`/modules/flips/deals/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  advanceFlipDealStage: (id: string, stage: string, note?: string) =>
    request<FlipDeal>(`/modules/flips/deals/${id}/stage`, {
      method: "POST",
      auth: true,
      body: { stage, note },
    }),
  underwriteFlipDeal: (id: string, body: UnderwriteInput) =>
    request<DealUnderwriting>(`/modules/flips/deals/${id}/underwrite`, {
      method: "POST",
      auth: true,
      body,
    }),
  updateFlipChecklist: (id: string, checklist: DealChecklistItem[]) =>
    request<FlipDeal>(`/modules/flips/deals/${id}/checklist`, {
      method: "PATCH",
      auth: true,
      body: { checklist },
    }),
  convertFlipDeal: (id: string) =>
    request<ConvertDealResponse>(`/modules/flips/deals/${id}/convert`, {
      method: "POST",
      auth: true,
    }),

  // ---- integrations: credential vault, notification log ----
  integrationSecrets: () =>
    request<IntegrationSecret[]>("/integrations/secrets", { auth: true }),
  setIntegrationSecret: (key: string, value: string) =>
    request<IntegrationSecret>("/integrations/secrets", {
      method: "PUT",
      auth: true,
      body: { key, value },
    }),
  deleteIntegrationSecret: (key: string) =>
    request<{ deleted: boolean }>(
      `/integrations/secrets/${encodeURIComponent(key)}`,
      { method: "DELETE", auth: true }
    ),
  notifications: (limit = 100) =>
    request<NotificationEntry[]>(`/integrations/notifications?limit=${limit}`, {
      auth: true,
    }),

  // ---- notification delivery providers (end-user configurable) ----
  notificationProviders: () =>
    request<NotificationProvider[]>("/integrations/providers", { auth: true }),
  createNotificationProvider: (body: CreateNotificationProviderInput) =>
    request<NotificationProvider>("/integrations/providers", {
      method: "POST",
      auth: true,
      body,
    }),
  updateNotificationProvider: (
    id: string,
    body: UpdateNotificationProviderInput
  ) =>
    request<NotificationProvider>(`/integrations/providers/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  deleteNotificationProvider: (id: string) =>
    request<{ deleted: boolean }>(`/integrations/providers/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  testNotificationProvider: (id: string, to?: string) =>
    request<{ queued: boolean; job_id: string }>(
      `/integrations/providers/${id}/test`,
      { method: "POST", auth: true, body: { to } }
    ),

  // ---- message templates (platform catalog + workspace copies) ----
  notificationTemplates: () =>
    request<NotificationTemplate[]>("/integrations/templates", { auth: true }),
  updateNotificationTemplate: (
    key: string,
    body: { subject?: string; body?: string; sms?: string }
  ) =>
    request<NotificationTemplate>(
      `/integrations/templates/${encodeURIComponent(key)}`,
      { method: "PUT", auth: true, body }
    ),
  resetNotificationTemplate: (key: string) =>
    request<{ reset: boolean; key: string }>(
      `/integrations/templates/${encodeURIComponent(key)}`,
      { method: "DELETE", auth: true }
    ),
  importNotificationTemplates: () =>
    request<{ imported: number; total: number }>(
      "/integrations/templates/import",
      { method: "POST", auth: true }
    ),

  // ---- in-app inbox + web push ----
  inbox: (limit = 50) =>
    request<InboxEntry[]>(`/notifications/inbox?limit=${limit}`, {
      auth: true,
    }),
  unreadCount: () =>
    request<{ unread: number }>("/notifications/unread_count", { auth: true }),
  markNotificationRead: (id: string) =>
    request<InboxEntry>(`/notifications/${id}/read`, {
      method: "POST",
      auth: true,
    }),
  markAllNotificationsRead: () =>
    request<{ marked: number }>("/notifications/read_all", {
      method: "POST",
      auth: true,
    }),
  vapidKey: () =>
    request<{ key: string }>("/notifications/vapid_key", { auth: true }),
  subscribePush: (body: { endpoint: string; p256dh: string; auth: string }) =>
    request<{ subscribed: boolean }>("/notifications/push_subscriptions", {
      method: "POST",
      auth: true,
      body,
    }),
  unsubscribePush: (endpoint: string) =>
    request<{ unsubscribed: boolean }>(
      `/notifications/push_subscriptions?endpoint=${encodeURIComponent(endpoint)}`,
      { method: "DELETE", auth: true }
    ),
  testPush: () =>
    request<{ queued: boolean; job_id: string }>("/notifications/test_push", {
      method: "POST",
      auth: true,
    }),

  // ---- documents (object storage) ----
  documents: (params: { owner_type?: string; owner_id?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.owner_type) qs.set("owner_type", params.owner_type);
    if (params.owner_id) qs.set("owner_id", params.owner_id);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<DocumentEntry[]>(`/documents${suffix}`, { auth: true });
  },
  registerDocument: (body: RegisterDocumentInput) =>
    request<UploadDocumentResponse>("/documents", {
      method: "POST",
      auth: true,
      body,
    }),
  documentDownloadUrl: (id: string) =>
    request<{ url: string; expires_at: string }>(`/documents/${id}/download`, {
      auth: true,
    }),
  deleteDocument: (id: string) =>
    request<{ deleted: boolean }>(`/documents/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  /** Update a document's filing metadata (category, wet-ink flag, and where the
   *  wet-ink original is stored). */
  updateDocument: (id: string, body: UpdateDocumentInput) =>
    request<DocumentEntry>(`/documents/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  /**
   * Full upload flow: register the metadata, then PUT the bytes straight to
   * the signed URL the backend minted (local store or S3 — same contract).
   */
  uploadDocument: async (
    meta: Omit<RegisterDocumentInput, "size_bytes">,
    file: File | Blob
  ): Promise<DocumentEntry> => {
    const reg = await api.registerDocument({ ...meta, size_bytes: file.size });
    const res = await fetch(reg.upload_url, {
      method: "PUT",
      body: file,
      headers: { "Content-Type": meta.mime_type },
    });
    if (!res.ok) {
      throw new ApiError(res.status, "upload_failed", "file upload failed");
    }
    return reg.document;
  },

  // ---- property media: photos / floorplans + hero ----
  propertyMedia: (id: string) =>
    request<PropertyMedia>(`/properties/${id}/media`, { auth: true }),
  setPropertyHero: (id: string, document_id: string | null) =>
    request<PropertyMedia>(`/properties/${id}/hero`, {
      method: "PATCH",
      auth: true,
      body: { document_id },
    }),

  // ---- rehab / construction ----
  rehabProjects: (propertyId: string) =>
    request<RehabProject[]>(`/properties/${propertyId}/rehab-projects`, {
      auth: true,
    }),
  createRehabProject: (propertyId: string, body: CreateRehabProjectInput) =>
    request<RehabProjectDetail>(`/properties/${propertyId}/rehab-projects`, {
      method: "POST",
      auth: true,
      body,
    }),
  rehabProject: (id: string) =>
    request<RehabProjectDetail>(`/rehab-projects/${id}`, { auth: true }),
  updateRehabProject: (id: string, body: Record<string, unknown>) =>
    request<RehabProjectDetail>(`/rehab-projects/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  createRehabLine: (
    projectId: string,
    body: { category: string; description?: string; budget_cents?: number }
  ) =>
    request<RehabProjectDetail>(`/rehab-projects/${projectId}/lines`, {
      method: "POST",
      auth: true,
      body,
    }),
  deleteRehabLine: (id: string) =>
    request<RehabProjectDetail>(`/rehab-lines/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  createChangeOrder: (
    projectId: string,
    body: { description: string; amount_cents: number }
  ) =>
    request<RehabProjectDetail>(`/rehab-projects/${projectId}/change-orders`, {
      method: "POST",
      auth: true,
      body,
    }),
  decideChangeOrder: (id: string, approve: boolean) =>
    request<RehabProjectDetail>(`/rehab-change-orders/${id}/decide`, {
      method: "POST",
      auth: true,
      body: { approve },
    }),
  createRehabDraw: (
    projectId: string,
    body: {
      title: string;
      amount_cents: number;
      contractor_id?: string;
      notes?: string;
    }
  ) =>
    request<RehabProjectDetail>(`/rehab-projects/${projectId}/draws`, {
      method: "POST",
      auth: true,
      body,
    }),
  rehabDraw: (id: string) =>
    request<RehabDrawDetail>(`/rehab-draws/${id}`, { auth: true }),
  setDrawStatus: (id: string, status: string) =>
    request<RehabProjectDetail>(`/rehab-draws/${id}/status`, {
      method: "PATCH",
      auth: true,
      body: { status },
    }),
  createLienWaiver: (
    drawId: string,
    body: {
      waiver_type: string;
      contractor_name?: string;
      amount_cents?: number;
      through_date?: string;
    }
  ) =>
    request<RehabDrawDetail>(`/rehab-draws/${drawId}/lien-waivers`, {
      method: "POST",
      auth: true,
      body,
    }),
  updateLienWaiver: (id: string, status: string) =>
    request<RehabDrawDetail>(`/rehab-lien-waivers/${id}`, {
      method: "PATCH",
      auth: true,
      body: { status },
    }),

  // ---- standard PM reports (Phase 8) ----
  rentRoll: (params: { property_id?: string; portfolio_id?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.property_id) qs.set("property_id", params.property_id);
    if (params.portfolio_id) qs.set("portfolio_id", params.portfolio_id);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<RentRollResp>(`/reports/rent-roll${suffix}`, { auth: true });
  },
  t12Report: (entity: string) =>
    request<T12Resp>(`/reports/t12?entity=${encodeURIComponent(entity)}`, {
      auth: true,
    }),
  agingReport: () => request<AgingResp>("/reports/aging", { auth: true }),
  delinquencyReport: () =>
    request<DelinquencyResp>("/reports/delinquency", { auth: true }),
  ownerStatement: (entity: string, from?: string, to?: string) => {
    const qs = new URLSearchParams({ entity });
    if (from) qs.set("from", from);
    if (to) qs.set("to", to);
    return request<OwnerStatementResp>(`/reports/owner-statement?${qs}`, {
      auth: true,
    });
  },
  tax1099: (year?: string) => {
    const qs = year ? `?year=${encodeURIComponent(year)}` : "";
    return request<Tax1099Resp>(`/reports/1099${qs}`, { auth: true });
  },

  // ---- global search (Phase 8) ----
  search: (q: string) =>
    request<SearchResp>(`/search?q=${encodeURIComponent(q)}`, { auth: true }),

  // ---- SaaS billing: workspace self-serve (Phase 8) ----
  billingSubscription: () =>
    request<BillingSubscription>("/billing/subscription", { auth: true }),
  billingInvoices: () =>
    request<PlatformInvoice[]>("/billing/invoices", { auth: true }),
  billingInvoice: (id: string) =>
    request<PlatformInvoice>(`/billing/invoices/${id}`, { auth: true }),
  /** Fetch a report export (CSV/PDF) as an authenticated blob for download. */
  downloadReport: async (path: string): Promise<Blob> => {
    const headers: Record<string, string> = {};
    const token = tokenStore.access;
    if (token) headers["Authorization"] = `Bearer ${token}`;
    const acting = actingTenant.get();
    if (acting) headers["X-Tenant"] = acting;
    const res = await fetch(`${API_BASE}${path}`, {
      headers,
      cache: "no-store",
    });
    if (!res.ok) {
      throw new ApiError(res.status, "export_failed", "report export failed");
    }
    return res.blob();
  },

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

  // ---- e-signature envelopes ----
  leaseEnvelope: (leaseId: string) =>
    request<EsignEnvelope>(`/leases/${leaseId}/envelope`, { auth: true }),
  createEnvelope: (
    leaseId: string,
    body: { message?: string; signers?: EsignSignerInput[] }
  ) =>
    request<CreateEnvelopeResponse>(`/leases/${leaseId}/envelope`, {
      method: "POST",
      auth: true,
      body,
    }),
  remindEnvelope: (envelopeId: string) =>
    request<RemindEnvelopeResponse>(`/esign/envelopes/${envelopeId}/remind`, {
      method: "POST",
      auth: true,
    }),
  voidEnvelope: (envelopeId: string, reason?: string) =>
    request<EsignEnvelope>(`/esign/envelopes/${envelopeId}/void`, {
      method: "POST",
      auth: true,
      body: { reason },
    }),
  // Public signer endpoints (tokenized link — no auth).
  publicSignView: (token: string, tenant = DEFAULT_TENANT) =>
    request<PublicSignView>(`/public/sign/${token}`, { tenant }),
  publicSign: (token: string, signed_name: string, tenant = DEFAULT_TENANT) =>
    request<PublicSignView>(`/public/sign/${token}`, {
      method: "POST",
      body: { signed_name, consent: true },
      tenant,
    }),
  publicDeclineSign: (
    token: string,
    reason: string | undefined,
    tenant = DEFAULT_TENANT
  ) =>
    request<PublicSignView>(`/public/sign/${token}/decline`, {
      method: "POST",
      body: { reason },
      tenant,
    }),
  publicMarkViewed: (token: string, tenant = DEFAULT_TENANT) =>
    request<PublicSignView>(`/public/sign/${token}/viewed`, {
      method: "POST",
      tenant,
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

  // ---- accounting: ledger, reports, finance series (Phase 3) ----
  ledgerAccounts: (entityId: string) =>
    request<LedgerAccount[]>(`/accounting/accounts?entity=${entityId}`, {
      auth: true,
    }),
  createLedgerAccount: (body: CreateLedgerAccountInput) =>
    request<LedgerAccount>("/accounting/accounts", {
      method: "POST",
      auth: true,
      body,
    }),
  ledgerTransactions: (entityId: string, limit = 50) =>
    request<LedgerTxn[]>(
      `/accounting/transactions?entity=${entityId}&limit=${limit}`,
      { auth: true }
    ),
  postLedgerTransaction: (body: ManualTxnInput) =>
    request<{ id: string }>("/accounting/transactions", {
      method: "POST",
      auth: true,
      body,
    }),
  trialBalance: (entityId: string) =>
    request<TrialBalance>(`/accounting/trial-balance?entity=${entityId}`, {
      auth: true,
    }),
  incomeStatement: (entityId: string, from?: string, to?: string) => {
    const qs = new URLSearchParams({ entity: entityId });
    if (from) qs.set("from", from);
    if (to) qs.set("to", to);
    return request<IncomeStatement>(`/accounting/income-statement?${qs}`, {
      auth: true,
    });
  },
  trustReconciliation: (entityId: string) =>
    request<TrustReconciliation>(
      `/accounting/trust-reconciliation?entity=${entityId}`,
      { auth: true }
    ),
  financeSeries: (months = 12) =>
    request<FinanceSeries>(`/finance/series?months=${months}`, { auth: true }),

  // ---- payments: back-office visibility ----
  payments: (params: { status?: string; lease?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.status) qs.set("status", params.status);
    if (params.lease) qs.set("lease", params.lease);
    const suffix = qs.toString() ? `?${qs.toString()}` : "";
    return request<Payment[]>(`/payments${suffix}`, { auth: true });
  },
  leasePaymentMethods: (leaseId: string) =>
    request<PaymentMethod[]>(`/leases/${leaseId}/payment-methods`, {
      auth: true,
    }),

  // ---- renter portal: my lease + pay rent + autopay ----
  myLease: () => request<MyLease>("/my/lease", { auth: true }),
  addMyPaymentMethod: (body: AddPaymentMethodInput) =>
    request<PaymentMethod>("/my/payment-methods", {
      method: "POST",
      auth: true,
      body,
    }),
  removeMyPaymentMethod: (id: string) =>
    request<{ removed: boolean }>(`/my/payment-methods/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  payMyLease: (body: PayInput) =>
    request<Payment>("/my/payments", { method: "POST", auth: true, body }),
  setMyAutopay: (methodId: string, day?: number) =>
    request<PaymentMethod>("/my/autopay", {
      method: "PUT",
      auth: true,
      body: { method_id: methodId, day },
    }),
  cancelMyAutopay: () =>
    request<{ cancelled: boolean }>("/my/autopay", {
      method: "DELETE",
      auth: true,
    }),

  // ---- renter portal: lease documents, maintenance, messages (Phase 5) ----
  myDocuments: () => request<DocumentEntry[]>("/my/documents", { auth: true }),
  myDocumentDownloadUrl: (id: string) =>
    request<{ url: string; expires_at: string }>(
      `/my/documents/${id}/download`,
      { auth: true }
    ),
  myTickets: () => request<MaintenanceTicket[]>("/my/tickets", { auth: true }),
  createMyTicket: (body: CreateMyTicketInput) =>
    request<MaintenanceTicket>("/my/tickets", {
      method: "POST",
      auth: true,
      body,
    }),
  myTicket: (id: string) =>
    request<MyTicketDetail>(`/my/tickets/${id}`, { auth: true }),
  addMyTicketComment: (id: string, body: string) =>
    request<TicketComment>(`/my/tickets/${id}/comments`, {
      method: "POST",
      auth: true,
      body: { body },
    }),
  /** Register a photo against the resident's request, then PUT the bytes to
   *  the signed URL (same two-step contract as the staff document service). */
  uploadMyTicketPhoto: async (
    id: string,
    file: File | (Blob & { name?: string })
  ): Promise<DocumentEntry> => {
    const mime = file.type || "application/octet-stream";
    const reg = await request<UploadDocumentResponse>(
      `/my/tickets/${id}/photos`,
      {
        method: "POST",
        auth: true,
        body: {
          filename: (file as File).name ?? "photo.jpg",
          mime_type: mime,
          size_bytes: file.size,
        },
      }
    );
    const res = await fetch(reg.upload_url, {
      method: "PUT",
      body: file,
      headers: { "Content-Type": mime },
    });
    if (!res.ok) {
      throw new ApiError(res.status, "upload_failed", "file upload failed");
    }
    return reg.document;
  },
  myThreads: () => request<MessageThread[]>("/my/messages", { auth: true }),
  createMyThread: (body: { subject: string; body: string }) =>
    request<MessageThreadDetail>("/my/messages", {
      method: "POST",
      auth: true,
      body,
    }),
  myThread: (id: string) =>
    request<MessageThreadDetail>(`/my/messages/${id}`, { auth: true }),
  replyMyThread: (id: string, body: string) =>
    request<ThreadMessage>(`/my/messages/${id}`, {
      method: "POST",
      auth: true,
      body: { body },
    }),
  reviewMyTicket: (id: string, rating: number, comment?: string) =>
    request<MaintenanceTicket>(`/my/tickets/${id}/review`, {
      method: "POST",
      auth: true,
      body: { rating, comment },
    }),
  myInspections: () =>
    request<InspectionDetail[]>("/my/inspections", { auth: true }),
  myDeposit: () => request<LeaseDeposit>("/my/deposit", { auth: true }),

  // ---- resident messaging (console) ----
  messageThreads: (status?: string) => {
    const suffix = status ? `?status=${status}` : "";
    return request<MessageThread[]>(`/messages${suffix}`, { auth: true });
  },
  messageThread: (id: string) =>
    request<MessageThreadDetail>(`/messages/${id}`, { auth: true }),
  replyMessageThread: (id: string, body: string) =>
    request<ThreadMessage>(`/messages/${id}/reply`, {
      method: "POST",
      auth: true,
      body: { body },
    }),
  updateMessageThread: (id: string, status: "open" | "closed") =>
    request<MessageThread>(`/messages/${id}`, {
      method: "PATCH",
      auth: true,
      body: { status },
    }),

  // ---- tenant lifecycle: inspections + deposit disposition ----
  leaseInspections: (leaseId: string) =>
    request<Inspection[]>(`/leases/${leaseId}/inspections`, { auth: true }),
  createInspection: (leaseId: string, body: CreateInspectionInput) =>
    request<InspectionDetail>(`/leases/${leaseId}/inspections`, {
      method: "POST",
      auth: true,
      body,
    }),
  inspection: (id: string) =>
    request<InspectionDetail>(`/inspections/${id}`, { auth: true }),
  updateInspection: (
    id: string,
    body: { scheduled_date?: string; notes?: string }
  ) =>
    request<InspectionDetail>(`/inspections/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  completeInspection: (id: string) =>
    request<InspectionDetail>(`/inspections/${id}/complete`, {
      method: "POST",
      auth: true,
      body: {},
    }),
  addInspectionItem: (id: string, body: { area: string; item: string }) =>
    request<InspectionItem>(`/inspections/${id}/items`, {
      method: "POST",
      auth: true,
      body,
    }),
  updateInspectionItem: (
    id: string,
    body: { condition?: string; notes?: string }
  ) =>
    request<InspectionItem>(`/inspection-items/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  deleteInspectionItem: (id: string) =>
    request<{ deleted: boolean }>(`/inspection-items/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  leaseDeposit: (leaseId: string) =>
    request<LeaseDeposit>(`/leases/${leaseId}/deposit`, { auth: true }),
  saveDepositDisposition: (leaseId: string, body: DispositionInput) =>
    request<DepositDisposition>(`/leases/${leaseId}/deposit/disposition`, {
      method: "PUT",
      auth: true,
      body,
    }),
  finalizeDepositDisposition: (id: string) =>
    request<DepositDisposition>(`/deposit-dispositions/${id}/finalize`, {
      method: "POST",
      auth: true,
      body: {},
    }),

  // ---- bank feeds + reconciliation ----
  allBankAccounts: (entityId?: string) => {
    const suffix = entityId ? `?entity=${entityId}` : "";
    return request<BankAccount[]>(`/bank-accounts${suffix}`, { auth: true });
  },
  linkBankAccount: (id: string, publicToken?: string) =>
    request<BankAccount>(`/bank-accounts/${id}/link`, {
      method: "POST",
      auth: true,
      body: { public_token: publicToken },
    }),
  syncBankAccount: (id: string) =>
    request<{ queued: boolean; job_id: string }>(`/bank-accounts/${id}/sync`, {
      method: "POST",
      auth: true,
      body: {},
    }),
  bankTransactions: (accountId: string, status?: string) => {
    const suffix = status ? `?status=${status}` : "";
    return request<BankTxn[]>(
      `/bank-accounts/${accountId}/transactions${suffix}`,
      { auth: true }
    );
  },
  matchBankTransaction: (txnId: string, paymentId: string) =>
    request<BankTxn>(`/bank-transactions/${txnId}/match`, {
      method: "POST",
      auth: true,
      body: { payment_id: paymentId },
    }),
  ignoreBankTransaction: (txnId: string) =>
    request<BankTxn>(`/bank-transactions/${txnId}/ignore`, {
      method: "POST",
      auth: true,
      body: {},
    }),

  // ---- owner payouts ----
  payouts: () => request<Payout[]>("/payouts", { auth: true }),
  computePayout: (body: ComputePayoutInput) =>
    request<Payout>("/payouts/compute", { method: "POST", auth: true, body }),
  executePayout: (id: string) =>
    request<Payout>(`/payouts/${id}/execute`, {
      method: "POST",
      auth: true,
      body: {},
    }),

  // ---- accounts payable (vendor bills) ----
  payables: (params: { status?: string; counterparty?: string } = {}) => {
    const qs = new URLSearchParams();
    if (params.status) qs.set("status", params.status);
    if (params.counterparty) qs.set("counterparty", params.counterparty);
    const suffix = qs.size ? `?${qs.toString()}` : "";
    return request<VendorBill[]>(`/payables${suffix}`, { auth: true });
  },
  payable: (id: string) =>
    request<VendorBill>(`/payables/${id}`, { auth: true }),
  createPayable: (body: CreateVendorBillInput) =>
    request<VendorBill>("/payables", { method: "POST", auth: true, body }),
  updatePayable: (id: string, body: UpdateVendorBillInput) =>
    request<VendorBill>(`/payables/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  submitPayable: (id: string) =>
    request<VendorBill>(`/payables/${id}/submit`, {
      method: "POST",
      auth: true,
      body: {},
    }),
  approvePayable: (id: string) =>
    request<VendorBill>(`/payables/${id}/approve`, {
      method: "POST",
      auth: true,
      body: {},
    }),
  rejectPayable: (id: string, reason?: string) =>
    request<VendorBill>(`/payables/${id}/reject`, {
      method: "POST",
      auth: true,
      body: { reason },
    }),
  voidPayable: (id: string) =>
    request<VendorBill>(`/payables/${id}/void`, {
      method: "POST",
      auth: true,
      body: {},
    }),
  payPayable: (id: string) =>
    request<VendorBill>(`/payables/${id}/pay`, {
      method: "POST",
      auth: true,
      body: {},
    }),

  // ---- calendar / reminders ----
  reminders: (
    params: {
      from?: string;
      to?: string;
      subject_type?: string;
      status?: string;
    } = {}
  ) => {
    const qs = new URLSearchParams();
    if (params.from) qs.set("from", params.from);
    if (params.to) qs.set("to", params.to);
    if (params.subject_type) qs.set("subject_type", params.subject_type);
    if (params.status) qs.set("status", params.status);
    const suffix = qs.size ? `?${qs.toString()}` : "";
    return request<Reminder[]>(`/reminders${suffix}`, { auth: true });
  },
  createReminder: (body: CreateReminderInput) =>
    request<Reminder>("/reminders", { method: "POST", auth: true, body }),
  updateReminder: (id: string, body: UpdateReminderInput) =>
    request<Reminder>(`/reminders/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  deleteReminder: (id: string) =>
    request<{ deleted: boolean }>(`/reminders/${id}`, {
      method: "DELETE",
      auth: true,
    }),

  // ---- CRM leads (inbound leasing email lands here) ----
  leads: (status?: string) => {
    const suffix = status ? `?status=${status}` : "";
    return request<LeadsResponse>(`/leads${suffix}`, { auth: true });
  },
  updateLead: (id: string, body: UpdateLeadInput) =>
    request<Lead>(`/leads/${id}`, { method: "PATCH", auth: true, body }),

  // ---- email integration: inbound comms log + domain deliverability ----
  inboundEmails: () =>
    request<InboundEmailLog[]>("/integrations/inbound-emails", { auth: true }),
  verifyDomainEmail: (id: string) =>
    request<DomainInfo>(`/domains/${id}/verify-email`, {
      method: "POST",
      auth: true,
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
  /** Renter attributes — drive application auto-fill + conditional charges. */
  has_pet: boolean;
  pet_details: string | null;
  is_military: boolean;
  annual_income_cents: number | null;
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
  /** Renter attributes (application auto-fill). */
  has_pet?: boolean;
  pet_details?: string;
  is_military?: boolean;
  annual_income_cents?: number;
}

/** Everything the "My profile" page needs in one fetch. */
export interface MyProfileView {
  /** Account display name. */
  name: string;
  /** Account email (identity — applications always use this). */
  email: string;
  profile: ProfileDto;
  vehicles: VehicleProfile[];
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

export interface DealSensitivityPoint {
  rent_growth_bps: number;
  rent_growth_pct: number;
  irr_bps: number | null;
  irr_pct: number | null;
}

/** Computed underwriting for a deal. Money values pair `*_cents` with a
 * display `*_label`; rates are `*_bps` alongside a `*_pct` float. */
export interface DealUnderwriting {
  purchase_price_cents: number;
  purchase_price_label: string;
  total_project_cost_cents: number;
  total_project_cost_label: string;
  loan_amount_cents: number;
  loan_amount_label: string;
  down_payment_cents: number;
  down_payment_label: string;
  total_cash_invested_cents: number;
  total_cash_invested_label: string;
  monthly_debt_service_cents: number;
  monthly_debt_service_label: string;
  annual_debt_service_cents: number;
  annual_debt_service_label: string;
  gross_rent_annual_cents: number;
  gross_rent_annual_label: string;
  vacancy_loss_cents: number;
  vacancy_loss_label: string;
  effective_gross_income_cents: number;
  effective_gross_income_label: string;
  operating_expenses_annual_cents: number;
  operating_expenses_annual_label: string;
  noi_annual_cents: number;
  noi_annual_label: string;
  annual_cash_flow_cents: number;
  annual_cash_flow_label: string;
  cap_rate_bps: number;
  cap_rate_pct: number;
  cash_on_cash_bps: number;
  cash_on_cash_pct: number;
  dscr: number;
  exit_value_cents: number;
  exit_value_label: string;
  loan_balance_at_exit_cents: number;
  loan_balance_at_exit_label: string;
  net_sale_proceeds_cents: number;
  net_sale_proceeds_label: string;
  irr_bps: number | null;
  irr_pct: number | null;
  total_profit_cents: number;
  total_profit_label: string;
  sensitivity: DealSensitivityPoint[];
}

export interface DealChecklistItem {
  key: string;
  label: string;
  done: boolean;
  note?: string | null;
}

/** An acquisition deal with its computed underwriting and parsed checklist. */
export interface FlipDeal {
  id: string;
  name: string;
  address: string;
  city: string;
  stage: string;
  stage_label: string;
  strategy: string;
  property_type: string | null;
  source: string | null;
  broker_id: string | null;
  notes: string | null;
  asking_price_cents: number | null;
  asking_price_label: string | null;
  offer_price_cents: number | null;
  offer_price_label: string | null;
  earnest_money_cents: number | null;
  earnest_money_label: string | null;
  target_close_on: string | null;
  arv_cents: number | null;
  arv_label: string | null;
  rehab_budget_cents: number | null;
  rehab_budget_label: string | null;
  closing_costs_cents: number | null;
  est_monthly_rent_cents: number | null;
  est_monthly_rent_label: string | null;
  est_monthly_expenses_cents: number | null;
  vacancy_bps: number | null;
  down_payment_bps: number | null;
  interest_rate_bps: number | null;
  loan_term_years: number | null;
  rent_growth_bps: number | null;
  appreciation_bps: number | null;
  exit_cap_rate_bps: number | null;
  selling_costs_bps: number | null;
  hold_years: number | null;
  checklist: DealChecklistItem[];
  converted_property_id: string | null;
  created_at: string;
  updated_at: string;
  underwriting: DealUnderwriting;
}

export interface DealEvent {
  id: string;
  kind: string;
  from_stage: string | null;
  to_stage: string | null;
  body: string | null;
  actor_user_id: string | null;
  created_at: string;
}

export interface DealDetail extends FlipDeal {
  events: DealEvent[];
}

export interface FlipPipeline {
  preview: boolean;
  stages: FlipStage[];
  deals: FlipDeal[];
}

export interface CreateDealInput {
  name: string;
  address?: string;
  city?: string;
  strategy?: string;
  property_type?: string;
  source?: string;
  broker_id?: string;
  asking_price_cents?: number;
  offer_price_cents?: number;
  est_monthly_rent_cents?: number;
  rehab_budget_cents?: number;
  notes?: string;
}

/** Deal patch + underwriting assumptions. Every field optional. */
export interface UpdateDealInput {
  name?: string;
  address?: string;
  city?: string;
  strategy?: string;
  property_type?: string;
  source?: string;
  broker_id?: string;
  notes?: string;
  asking_price_cents?: number;
  offer_price_cents?: number;
  earnest_money_cents?: number;
  target_close_on?: string;
  arv_cents?: number;
  rehab_budget_cents?: number;
  closing_costs_cents?: number;
  est_monthly_rent_cents?: number;
  est_monthly_expenses_cents?: number;
  vacancy_bps?: number;
  down_payment_bps?: number;
  interest_rate_bps?: number;
  loan_term_years?: number;
  rent_growth_bps?: number;
  appreciation_bps?: number;
  exit_cap_rate_bps?: number;
  selling_costs_bps?: number;
  hold_years?: number;
}

/** Ad-hoc "what-if" overrides for the stateless underwrite endpoint. */
export type UnderwriteInput = Omit<
  UpdateDealInput,
  | "name"
  | "address"
  | "city"
  | "strategy"
  | "property_type"
  | "source"
  | "broker_id"
  | "notes"
  | "asking_price_cents"
  | "offer_price_cents"
  | "earnest_money_cents"
  | "target_close_on"
> & { purchase_price_cents?: number };

export interface ConvertDealResponse {
  deal: FlipDeal;
  property_id: string;
}

// ---- integrations: secrets, notifications, documents ----

/** A stored credential, masked — the plaintext is never sent back. */
export interface IntegrationSecret {
  id: string;
  key: string;
  last4: string;
  created_at: string;
  rotated_at: string | null;
}

export interface NotificationEntry {
  id: string;
  channel: string;
  template_key: string;
  recipient: string;
  status: string;
  provider_message_id: string | null;
  subject: string | null;
  body: string | null;
  last_error: string | null;
  created_at: string;
}

/** A configured delivery provider (credential masked to last4). */
export interface NotificationProvider {
  id: string;
  channel: string;
  kind: string;
  config: Record<string, unknown>;
  enabled: boolean;
  is_default: boolean;
  credential_last4: string | null;
  created_at: string;
}

export interface CreateNotificationProviderInput {
  channel: string;
  kind: string;
  config?: Record<string, unknown>;
  credential?: string;
  is_default?: boolean;
}

export interface UpdateNotificationProviderInput {
  config?: Record<string, unknown>;
  credential?: string;
  enabled?: boolean;
  is_default?: boolean;
}

/**
 * One notification message template: the effective fields (workspace copy
 * layered over the platform default) plus where each came from.
 */
export interface NotificationTemplate {
  key: string;
  /** Email subject; doubles as the push/in-app title. */
  subject: string;
  /** Long email body. */
  body: string;
  /** Short text used for SMS, chat, push, and in-app renditions. */
  sms: string;
  /** The workspace holds its own editable copy. */
  customized: boolean;
  /** A platform default exists (reset restores it). */
  has_default: boolean;
}

/** One in-app inbox entry for the signed-in user. */
export interface InboxEntry {
  id: string;
  template_key: string;
  subject: string | null;
  body: string | null;
  read_at: string | null;
  created_at: string;
}

export interface DocumentEntry {
  id: string;
  owner_type: string;
  owner_id: string;
  filename: string;
  category: string | null;
  requires_wet_ink: boolean;
  physical_location: string | null;
  mime_type: string;
  size_bytes: number;
  checksum: string | null;
  version: number;
  previous_version_id: string | null;
  status: string;
  retention_expires_at: string | null;
  created_at: string;
}

export interface PropertyMediaItem {
  document_id: string;
  filename: string;
  category: string | null;
  mime_type: string;
  size_bytes: number;
  /** Short-lived signed URL, renderable in an `<img>`. */
  url: string | null;
  is_hero: boolean;
  created_at: string;
}

export interface PropertyMedia {
  hero_document_id: string | null;
  hero_url: string | null;
  items: PropertyMediaItem[];
}

// ---- rehab / construction ----
export interface RehabLine {
  id: string;
  category: string;
  description: string | null;
  budget_cents: number;
  budget_label: string;
  sort_order: number;
}

export interface RehabChangeOrder {
  id: string;
  description: string;
  amount_cents: number;
  amount_label: string;
  status: string;
  created_at: string;
  decided_at: string | null;
}

export interface RehabDraw {
  id: string;
  project_id: string;
  number: number;
  title: string;
  amount_cents: number;
  amount_label: string;
  status: string;
  contractor_id: string | null;
  contractor_name: string | null;
  notes: string | null;
  funded_at: string | null;
  created_at: string;
}

export interface LienWaiver {
  id: string;
  draw_id: string;
  waiver_type: string;
  waiver_type_label: string;
  contractor_name: string;
  amount_cents: number;
  amount_label: string;
  through_date: string | null;
  status: string;
  document_id: string | null;
  created_at: string;
}

export interface RehabProject {
  id: string;
  property_id: string;
  name: string;
  status: string;
  base_budget_cents: number;
  base_budget_label: string;
  contingency_bps: number;
  contingency_pct: number;
  contingency_cents: number;
  contingency_label: string;
  adjusted_budget_cents: number;
  adjusted_budget_label: string;
  approved_change_orders_cents: number;
  approved_change_orders_label: string;
  drawn_cents: number;
  drawn_label: string;
  pending_draws_cents: number;
  pending_draws_label: string;
  remaining_cents: number;
  remaining_label: string;
  lines_budget_cents: number;
  lines_budget_label: string;
  start_date: string | null;
  target_end_date: string | null;
  notes: string | null;
  line_count: number;
  draw_count: number;
  created_at: string;
  updated_at: string;
}

export interface RehabProjectDetail extends RehabProject {
  lines: RehabLine[];
  draws: RehabDraw[];
  change_orders: RehabChangeOrder[];
}

export interface RehabDrawDetail extends RehabDraw {
  lien_waivers: LienWaiver[];
}

export interface CreateRehabProjectInput {
  name: string;
  budget_cents?: number;
  contingency_bps?: number;
  start_date?: string;
  target_end_date?: string;
  notes?: string;
}

// ---- standard PM reports (Phase 8) ----
export interface RentRollRow {
  property_name: string;
  unit: string;
  tenant_name: string;
  rent_cents: number;
  rent_label: string;
  term: string;
  status: string;
  payment_status: string;
  balance_cents: number;
  balance_label: string;
}
export interface RentRollResp {
  generated_at: string;
  rows: RentRollRow[];
  lease_count: number;
  total_rent_cents: number;
  total_rent_label: string;
  total_balance_cents: number;
  total_balance_label: string;
}

export interface T12Row {
  account_name: string;
  kind: string;
  monthly_cents: number[];
  total_cents: number;
  total_label: string;
}
export interface T12Resp {
  generated_at: string;
  entity_id: string;
  months: string[];
  income: T12Row[];
  expenses: T12Row[];
  income_totals_cents: number[];
  expense_totals_cents: number[];
  noi_totals_cents: number[];
  total_income_label: string;
  total_expense_label: string;
  net_cents: number;
  net_label: string;
}

export interface SearchHit {
  kind: string;
  id: string;
  title: string;
  subtitle: string;
  href: string;
}
export interface SearchResp {
  query: string;
  hits: SearchHit[];
}

// ---- SaaS billing (Phase 8) ----
export interface BillingPlan {
  key: string;
  name: string;
  base_cents: number;
  base_label: string;
  included_units: number;
  overage_cents: number;
  overage_label: string;
  blurb: string;
  features: string[];
  current: boolean;
}
export interface BillingEstimateLine {
  description: string;
  quantity: number;
  amount_cents: number;
  amount_label: string;
}
export interface BillingEstimate {
  unit_count: number;
  included_units: number;
  base_cents: number;
  base_label: string;
  overage_cents: number;
  overage_label: string;
  total_cents: number;
  total_label: string;
  lines: BillingEstimateLine[];
}
export interface BillingSubscription {
  plan: string;
  plan_name: string;
  status: string;
  properties: number;
  units: number;
  estimate: BillingEstimate;
  outstanding_cents: number;
  outstanding_label: string;
  plans: BillingPlan[];
}
export interface InvoiceLine {
  description: string;
  quantity: number;
  unit_price_cents: number;
  unit_price_label: string;
  amount_cents: number;
  amount_label: string;
}
export interface PlatformInvoice {
  id: string;
  tenant_id: string;
  period: string;
  plan: string;
  status: string;
  unit_count: number;
  included_units: number;
  base_cents: number;
  base_label: string;
  overage_cents: number;
  overage_label: string;
  total_cents: number;
  total_label: string;
  issued_at: string | null;
  due_date: string | null;
  paid_at: string | null;
  lines: InvoiceLine[];
}
export interface TenantBilling {
  tenant_id: string;
  name: string;
  slug: string;
  plan: string;
  status: string;
  units: number;
  mrr_cents: number;
  mrr_label: string;
  outstanding_cents: number;
  outstanding_label: string;
}
export interface BillingOverview {
  tenant_count: number;
  mrr_cents: number;
  mrr_label: string;
  outstanding_cents: number;
  outstanding_label: string;
  tenants: TenantBilling[];
}

export interface AgingBuckets {
  current_cents: number;
  d1_30_cents: number;
  d31_60_cents: number;
  d61_90_cents: number;
  over90_cents: number;
  total_cents: number;
}
export interface AgingRow extends AgingBuckets {
  tenant_name: string;
  property_name: string;
}
export interface AgingResp extends AgingBuckets {
  generated_at: string;
  rows: AgingRow[];
}

export interface DelinquencyRow {
  tenant_name: string;
  property_name: string;
  unit: string;
  payment_status: string;
  balance_cents: number;
  balance_label: string;
  days_late: number;
  oldest_due_date: string | null;
}
export interface DelinquencyResp {
  generated_at: string;
  rows: DelinquencyRow[];
  tenant_count: number;
  total_balance_cents: number;
  total_balance_label: string;
}

export interface StatementLine {
  name: string;
  amount_cents: number;
  amount_label: string;
}
export interface OwnerStatementResp {
  generated_at: string;
  entity_id: string;
  entity_name: string;
  period_start: string;
  period_end: string;
  rent_collected_cents: number;
  rent_collected_label: string;
  expense_lines: StatementLine[];
  expenses_cents: number;
  expenses_label: string;
  mgmt_fee_cents: number;
  mgmt_fee_label: string;
  net_cents: number;
  net_label: string;
}

export interface Recipient1099 {
  form: string;
  box_label: string;
  recipient_id: string;
  name: string;
  tin: string | null;
  address: string | null;
  amount_cents: number;
  amount_label: string;
}
export interface Tax1099Resp {
  generated_at: string;
  year: number;
  threshold_cents: number;
  threshold_label: string;
  nec: Recipient1099[];
  misc: Recipient1099[];
  nec_total_cents: number;
  nec_total_label: string;
  misc_total_cents: number;
  misc_total_label: string;
}

export interface RegisterDocumentInput {
  owner_type: string;
  owner_id: string;
  filename: string;
  mime_type: string;
  size_bytes?: number;
  retention_days?: number;
  category?: string;
  requires_wet_ink?: boolean;
  physical_location?: string;
}

export interface UpdateDocumentInput {
  category?: string;
  requires_wet_ink?: boolean;
  physical_location?: string;
}

export interface CategoryCount {
  category: string | null;
  count: number;
}

/** The property Documents tab: latest version of each doc + category tally +
 *  the wet-ink originals with their storage locations. */
export interface PropertyDocuments {
  property_id: string;
  total: number;
  documents: DocumentEntry[];
  categories: CategoryCount[];
  wet_ink_originals: DocumentEntry[];
}

export interface UploadDocumentResponse {
  document: DocumentEntry;
  upload_url: string;
  upload_url_expires_at: string;
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

// ---- e-signature envelopes ----

/** One party on an envelope, with their signing state + signature record. */
export interface EsignSigner {
  id: string;
  /** resident | landlord | guarantor | other */
  role: string;
  name: string;
  email: string;
  phone: string | null;
  /** sent | viewed | signed | declined */
  status: string;
  viewed_at: string | null;
  signed_at: string | null;
  signed_name: string | null;
  decline_reason: string | null;
}

/** One entry in the envelope's ESIGN/UETA audit trail. */
export interface EsignEvent {
  id: string;
  signer_id: string | null;
  /** sent | viewed | signed | declined | reminded | completed | voided */
  event: string;
  detail: Record<string, unknown>;
  ip: string | null;
  user_agent: string | null;
  created_at: string;
}

export interface EsignEnvelope {
  id: string;
  lease_id: string;
  lease_document_id: string;
  title: string;
  message: string | null;
  /** sent | partially_signed | completed | declined | voided */
  status: string;
  body_hash: string;
  signed_document_id: string | null;
  sent_at: string;
  completed_at: string | null;
  voided_at: string | null;
  void_reason: string | null;
  signers: EsignSigner[];
  events: EsignEvent[];
}

export interface EsignSignerInput {
  role?: string;
  name: string;
  email: string;
  phone?: string;
}

/** A freshly minted signing link — returned once, never retrievable again. */
export interface EsignSignerLink {
  signer_id: string;
  name: string;
  email: string;
  sign_url: string;
}

export interface CreateEnvelopeResponse {
  envelope: EsignEnvelope;
  sign_links: EsignSignerLink[];
}

export interface RemindEnvelopeResponse {
  reminded: number;
  sign_links: EsignSignerLink[];
}

export interface PublicCoSigner {
  name: string;
  role: string;
  status: string;
}

/** The public signing page's view, scoped to one signer's token. */
export interface PublicSignView {
  company: string;
  envelope_status: string;
  document_title: string;
  document_body: string | null;
  body_hash: string;
  message: string | null;
  signer: EsignSigner;
  co_signers: PublicCoSigner[];
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

export interface EmailDnsRecord {
  /** `spf` | `dkim` | `dmarc`. */
  key: string;
  /** The DNS name to create the TXT record at. */
  name: string;
  /** The TXT value to publish. */
  value: string;
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
  /** True once SPF + DKIM + DMARC all verified for branded sending. */
  email_verified: boolean;
  email_verified_at: string | null;
  /** Per-record email DNS check results, e.g. `{ spf: true, dkim: false }`. */
  email_dns_status: Record<string, boolean>;
  /** Records to publish for branded mail (custom domains only). */
  email_dns_records: EmailDnsRecord[] | null;
}

// ---- accounts payable (vendor bills, #58) ----

export interface VendorBillLineItem {
  description: string;
  amount_cents: number;
}

export interface VendorBill {
  id: string;
  bill_number: string;
  entity_id: string;
  entity_name: string | null;
  counterparty_id: string;
  vendor_name: string | null;
  property_id: string | null;
  maintenance_ticket_id: string | null;
  memo: string;
  line_items: VendorBillLineItem[];
  amount_cents: number;
  amount_label: string;
  due_date: string | null;
  /** draft | submitted | approved | processing | paid | failed | void */
  status: string;
  submitted_at: string | null;
  approved_at: string | null;
  rejected_reason: string | null;
  accrual_txn_id: string | null;
  payment_txn_id: string | null;
  failure_reason: string | null;
  paid_at: string | null;
  created_at: string;
}

export interface CreateVendorBillInput {
  counterparty_id?: string;
  entity_id?: string;
  property_id?: string;
  maintenance_ticket_id?: string;
  memo?: string;
  line_items?: VendorBillLineItem[];
  amount_cents?: number;
  due_date?: string;
}

export interface UpdateVendorBillInput {
  memo?: string;
  line_items?: VendorBillLineItem[];
  amount_cents?: number;
  due_date?: string;
}

// ---- calendar / reminders (#54) ----

export interface Reminder {
  id: string;
  /** lease | license | insurance | tour | inspection | custom */
  subject_type: string;
  subject_id: string | null;
  title: string;
  description: string | null;
  due_date: string;
  lead_days: number[];
  recipients: string[];
  fired: number[];
  /** active | done | cancelled */
  status: string;
  /** Days until due (negative = overdue). */
  days_left: number | null;
  completed_at: string | null;
  created_at: string;
}

export interface CreateReminderInput {
  subject_type: string;
  subject_id?: string;
  title: string;
  description?: string;
  due_date: string;
  lead_days?: number[];
  recipients?: string[];
}

export interface UpdateReminderInput {
  title?: string;
  description?: string;
  due_date?: string;
  lead_days?: number[];
  recipients?: string[];
  status?: string;
}

// ---- CRM leads (#46 seed, landed with #62) ----

export interface Lead {
  id: string;
  name: string;
  email: string;
  phone: string | null;
  source: string;
  /** new | contacted | toured | applied | closed */
  status: string;
  notes: string | null;
  last_message: string | null;
  created_at: string;
  updated_at: string;
}

export interface LeadsResponse {
  /** The monitored leasing inbox — mail sent here creates/updates leads. */
  inbox_address: string | null;
  leads: Lead[];
}

export interface UpdateLeadInput {
  name?: string;
  phone?: string;
  status?: string;
  notes?: string;
}

// ---- inbound email comms log (#62) ----

export interface InboundEmailLog {
  id: string;
  from_email: string;
  to_email: string;
  subject: string;
  body_text: string;
  /** ticket_comment | lead | unmatched */
  routed: string;
  routed_id: string | null;
  created_at: string;
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
  /** `plaid` once linked for feeds. */
  provider: string | null;
  linked: boolean;
  last_synced_at: string | null;
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

// ---- accounting & payments (Phase 3) ----

export interface LedgerAccount {
  id: string;
  entity_id: string;
  code: string;
  name: string;
  kind: string;
  subtype: string | null;
  is_trust: boolean;
  system: boolean;
  active: boolean;
  debit_cents: number;
  credit_cents: number;
  balance_cents: number;
  balance_label: string;
}

export interface CreateLedgerAccountInput {
  entity_id: string;
  code: string;
  name: string;
  kind: string;
  is_trust?: boolean;
}

export interface LedgerEntry {
  id: string;
  account_id: string;
  account_code: string;
  account_name: string;
  side: string;
  amount_cents: number;
  amount_label: string;
  property_id: string | null;
  lease_id: string | null;
}

export interface LedgerTxn {
  id: string;
  entity_id: string;
  txn_date: string;
  memo: string;
  source_type: string;
  source_id: string | null;
  posted_by: string | null;
  created_at: string;
  entries: LedgerEntry[];
}

export interface ManualTxnInput {
  entity_id: string;
  txn_date?: string;
  memo: string;
  legs: { account_id: string; side: string; amount_cents: number }[];
}

export interface TrialBalanceRow {
  code: string;
  name: string;
  kind: string;
  debit_cents: number;
  credit_cents: number;
  debit_label: string;
  credit_label: string;
}

export interface TrialBalance {
  entity_id: string;
  rows: TrialBalanceRow[];
  total_debits_cents: number;
  total_credits_cents: number;
  balanced: boolean;
}

export interface StatementLine {
  name: string;
  amount_cents: number;
  amount_label: string;
}

export interface IncomeStatement {
  entity_id: string;
  from: string | null;
  to: string | null;
  income: StatementLine[];
  expenses: StatementLine[];
  total_income_cents: number;
  total_expenses_cents: number;
  net_cents: number;
  net_label: string;
}

export interface TrustReconciliation {
  entity_id: string;
  trust_bank_cents: number;
  trust_liability_cents: number;
  difference_cents: number;
  trust_bank_label: string;
  trust_liability_label: string;
  reconciled: boolean;
}

export interface FinanceSeries {
  months: string[];
  rent_due_cents: number[];
  rent_collected_cents: number[];
  noi_cents: number[];
  occupancy_bps: number[];
  delinquency_bps: number[];
  portfolio_value_cents: number[];
  active_leases: number[];
}

export interface Payment {
  id: string;
  lease_id: string;
  kind: string;
  due_date: string;
  paid_date: string | null;
  amount_cents: number;
  amount_label: string;
  status: string;
  method: string | null;
  receipt_number: string | null;
  failure_reason: string | null;
  created_at: string;
}

export interface PaymentMethod {
  id: string;
  lease_id: string | null;
  provider: string;
  kind: string;
  brand: string | null;
  last4: string;
  exp_month: number | null;
  exp_year: number | null;
  status: string;
  autopay: boolean;
  autopay_day: number | null;
}

export interface AddPaymentMethodInput {
  kind: string;
  external_id?: string;
  last4?: string;
  brand?: string;
  exp_month?: number;
  exp_year?: number;
}

export interface PayInput {
  payment_id?: string;
  kind?: string;
  method_id: string;
}

export interface MyLease {
  lease_id: string;
  property_name: string;
  property_address: string;
  unit_label: string | null;
  tenant_name: string;
  start_date: string;
  end_date: string | null;
  status: string;
  payment_status: string;
  rent_cents: number;
  rent_label: string;
  balance_cents: number;
  balance_label: string;
  deposit_cents: number | null;
  deposit_label: string | null;
  deposit_paid: boolean;
  autopay_enabled: boolean;
  due_items: Payment[];
  history: Payment[];
  methods: PaymentMethod[];
}

export interface BankTxn {
  id: string;
  bank_account_id: string;
  posted_date: string;
  description: string;
  amount_cents: number;
  amount_label: string;
  status: string;
  matched_payment_id: string | null;
}

export interface Payout {
  id: string;
  entity_id: string;
  entity_name: string | null;
  period_start: string;
  period_end: string;
  rent_collected_cents: number;
  rent_collected_label: string;
  expenses_cents: number;
  expenses_label: string;
  mgmt_fee_cents: number;
  mgmt_fee_label: string;
  net_cents: number;
  net_label: string;
  status: string;
  statement_document_id: string | null;
  ledger_txn_id: string | null;
  failure_reason: string | null;
  created_at: string;
}

export interface ComputePayoutInput {
  entity_id: string;
  period_start: string;
  period_end: string;
}

// ---- Phase 5: resident portal round-out ----

export interface CreateMyTicketInput {
  title: string;
  description?: string;
  category?: string;
  priority?: string;
  location?: string;
  access_notes?: string;
  permission_to_enter?: boolean;
}

export interface MyTicketDetail extends MaintenanceTicket {
  comments: TicketComment[];
  documents: DocumentEntry[];
}

export interface ThreadMessage {
  id: string;
  thread_id: string;
  sender_user_id: string;
  sender_kind: "resident" | "staff";
  sender_name: string;
  body: string;
  created_at: string;
}

export interface MessageThread {
  id: string;
  lease_id: string;
  property_id: string;
  subject: string;
  status: "open" | "closed";
  last_message_at: string;
  created_at: string;
  resident_name: string | null;
  property_address: string | null;
  message_count: number;
  last_sender_kind: string | null;
  last_preview: string | null;
}

export interface MessageThreadDetail extends MessageThread {
  messages: ThreadMessage[];
}

export interface InspectionItem {
  id: string;
  inspection_id: string;
  area: string;
  item: string;
  condition: string;
  notes: string | null;
  sort_order: number;
}

export interface Inspection {
  id: string;
  lease_id: string;
  property_id: string;
  unit_id: string | null;
  kind: "move_in" | "move_out";
  status: "draft" | "completed";
  scheduled_date: string | null;
  completed_at: string | null;
  notes: string | null;
  item_count: number;
  rated_count: number;
  created_at: string;
}

export interface InspectionDetail extends Inspection {
  items: InspectionItem[];
}

export interface CreateInspectionInput {
  kind: "move_in" | "move_out";
  scheduled_date?: string;
  notes?: string;
  blank?: boolean;
}

export interface DepositDeduction {
  id: string;
  description: string;
  amount_cents: number;
  amount_label: string;
}

export interface DepositDisposition {
  id: string;
  lease_id: string;
  property_id: string;
  status: "draft" | "processing" | "closed" | "failed";
  deposit_cents: number;
  deposit_label: string;
  withheld_cents: number;
  withheld_label: string;
  refund_cents: number | null;
  refund_label: string | null;
  notes: string | null;
  failure_reason: string | null;
  statement_document_id: string | null;
  deductions: DepositDeduction[];
  finalized_at: string | null;
  closed_at: string | null;
  created_at: string;
}

export interface DispositionInput {
  deductions: { description: string; amount_cents: number }[];
  notes?: string;
}

export interface LeaseDeposit {
  lease_id: string;
  deposit_cents: number | null;
  deposit_label: string | null;
  deposit_paid: boolean;
  disposition: DepositDisposition | null;
}
