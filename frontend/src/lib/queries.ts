"use client";

// Typed TanStack Query hooks wrapping the existing `api` client. These are the
// canonical way to read server state from client components — prefer them over
// ad-hoc useEffect + useState. Query keys are centralised in `queryKeys` so
// they can be invalidated consistently after mutations.

import {
  useMutation,
  useQuery,
  useQueryClient,
  type UseQueryOptions,
} from "@tanstack/react-query";
import {
  api,
  iam,
  type AuditEntry,
  type CreateRoleInput,
  type CreateTokenResponse,
  type CreateUserInput,
  type FlipPipeline,
  type InviteMemberInput,
  type Member,
  type Membership,
  type MembershipInput,
  type ModuleInfo,
  type CreateTenantInput,
  type PermissionDef,
  type PlatformMetrics,
  type ProfileDto,
  type TenantDetail,
  type Theme,
  type UpdateTenantInput,
  type UpdateThemeInput,
  type ProfileInput,
  type ProfileType,
  type Role,
  type TenantSummary,
  type TokenSummary,
  type UpdateRoleInput,
  type UpdateUserInput,
  type UserDetail,
  type UserSummary,
} from "./api";
import { tokenStore } from "./api";
import { toast } from "sonner";
import type {
  Application,
  Counterparty,
  CounterpartyDetail,
  CounterpartyNote,
  CreateCounterpartyInput,
  CreateLeaseInput,
  CreateLienInput,
  CreateMortgageInput,
  CreateOwnershipInput,
  CreateTemplateInput,
  CreateTicketInput,
  CreateUnitInput,
  EnrichmentRun,
  EnrichResponse,
  GenerateDocumentInput,
  GeneratedDocument,
  Lease,
  LeaseDetail,
  LeasePayment,
  Lien,
  Llc,
  LlcBranding,
  LlcDocument,
  LlcGroup,
  LlcTemplate,
  MaintenanceTicket,
  Mortgage,
  Ownership,
  PortfolioSummary,
  Property,
  PropertyIntel,
  PropertyProfile,
  RecordPaymentInput,
  StorageConfig,
  TicketDetail,
  Unit,
  UpdateLlcInput,
  UpdateStorageConfigInput,
  UpdateTicketInput,
  Workflow,
} from "./types";

/** Centralised, hierarchical query keys. */
export const queryKeys = {
  modules: ["modules"] as const,
  properties: ["properties"] as const,
  property: (id: string) => ["properties", id] as const,
  portfolioSummary: ["portfolio", "summary"] as const,
  applications: ["applications"] as const,
  apiTokens: ["api-tokens"] as const,
  // Property intelligence (enrichment)
  propertyIntel: (id: string) => ["properties", id, "intel"] as const,
  propertyEnrichment: (id: string) =>
    ["properties", id, "enrichment"] as const,
  // Investment workflow
  workflow: (propertyId: string) =>
    ["properties", propertyId, "workflow"] as const,
  // Rentals: units
  units: (propertyId: string) => ["properties", propertyId, "units"] as const,
  // Rentals: leases
  leases: (params?: { status?: string; property_id?: string }) =>
    ["leases", params ?? {}] as const,
  propertyLeases: (propertyId: string) =>
    ["properties", propertyId, "leases"] as const,
  lease: (id: string) => ["leases", "detail", id] as const,
  // Maintenance: tickets
  tickets: (params?: {
    status?: string;
    property_id?: string;
    priority?: string;
  }) => ["tickets", params ?? {}] as const,
  propertyTickets: (propertyId: string) =>
    ["properties", propertyId, "tickets"] as const,
  ticket: (id: string) => ["tickets", "detail", id] as const,
  // Title: ownership + liens
  ownership: (propertyId: string) =>
    ["properties", propertyId, "ownership"] as const,
  liens: (propertyId: string) => ["properties", propertyId, "liens"] as const,
  // Financing: mortgages
  mortgages: (propertyId: string) =>
    ["properties", propertyId, "mortgages"] as const,
  // Entities (counterparties)
  entities: (kind?: string) => ["entities", kind ?? ""] as const,
  entity: (id: string) => ["entities", "detail", id] as const,
  // LLCs
  llcGroups: ["portfolio", "llcs"] as const,
  llc: (id: string) => ["llcs", id] as const,
  llcDocuments: (id: string) => ["llcs", id, "documents"] as const,
  llcBranding: (id: string) => ["llcs", id, "branding"] as const,
  llcTemplates: (id: string) => ["llcs", id, "templates"] as const,
  generatedDocuments: (id: string) => ["llcs", id, "generated"] as const,
  // Storage configuration
  storageConfig: ["storage", "config"] as const,
  // Flips module
  flipPipeline: ["modules", "flips", "pipeline"] as const,
  // Platform (staff)
  platformTenants: ["platform", "tenants"] as const,
  platformTenant: (id: string) => ["platform", "tenants", id] as const,
  platformMetrics: ["platform", "metrics"] as const,
  tenantTheme: ["tenant-theme"] as const,
  // IAM
  permissionsCatalog: ["iam", "permissions"] as const,
  profileTypes: ["iam", "profile-types"] as const,
  roles: (params?: { scope?: string; tenant_id?: string }) =>
    ["iam", "roles", params ?? {}] as const,
  users: (q?: string) => ["iam", "users", q ?? ""] as const,
  user: (id: string) => ["iam", "users", "detail", id] as const,
  members: ["iam", "members"] as const,
  audit: (params?: { limit?: number; action?: string }) =>
    ["iam", "audit", params ?? {}] as const,
};

/** True when there's an access token to authenticate console requests. */
function isAuthed() {
  return !!tokenStore.access;
}

type QueryOpts<T> = Omit<
  UseQueryOptions<T, Error, T, readonly unknown[]>,
  "queryKey" | "queryFn"
>;

export function useModules(opts?: QueryOpts<ModuleInfo[]>) {
  return useQuery({
    queryKey: queryKeys.modules,
    queryFn: () => api.modules(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useProperties(opts?: QueryOpts<Property[]>) {
  return useQuery({
    queryKey: queryKeys.properties,
    queryFn: () => api.properties(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function usePortfolioSummary(opts?: QueryOpts<PortfolioSummary>) {
  return useQuery({
    queryKey: queryKeys.portfolioSummary,
    queryFn: () => api.portfolioSummary(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useApplications(opts?: QueryOpts<Application[]>) {
  return useQuery({
    queryKey: queryKeys.applications,
    queryFn: () => api.applications(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useApiTokens(opts?: QueryOpts<TokenSummary[]>) {
  return useQuery({
    queryKey: queryKeys.apiTokens,
    queryFn: () => api.apiTokens(),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Create an API token, then invalidate the token list. */
export function useCreateApiToken() {
  const qc = useQueryClient();
  return useMutation<
    CreateTokenResponse,
    Error,
    { name: string; scopes: string[] }
  >({
    mutationFn: ({ name, scopes }) => api.createApiToken(name, scopes),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.apiTokens });
    },
  });
}

/** Revoke an API token, then invalidate the token list. */
export function useRevokeApiToken() {
  const qc = useQueryClient();
  return useMutation<void, Error, string>({
    mutationFn: (id) => api.revokeApiToken(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.apiTokens });
    },
  });
}

// ---- IAM: queries ------------------------------------------------------------

/** The full catalog of grantable permissions (for the role matrix). */
export function usePermissionsCatalog(opts?: QueryOpts<PermissionDef[]>) {
  return useQuery({
    queryKey: queryKeys.permissionsCatalog,
    queryFn: () => iam.permissions(),
    enabled: isAuthed(),
    staleTime: 5 * 60_000,
    ...opts,
  });
}

/** Available personas / profile types. */
export function useProfileTypes(opts?: QueryOpts<ProfileType[]>) {
  return useQuery({
    queryKey: queryKeys.profileTypes,
    queryFn: () => iam.profileTypes(),
    enabled: isAuthed(),
    staleTime: 5 * 60_000,
    ...opts,
  });
}

/** Roles, optionally filtered by scope / tenant. */
export function useRoles(
  params?: { scope?: string; tenant_id?: string },
  opts?: QueryOpts<Role[]>
) {
  return useQuery({
    queryKey: queryKeys.roles(params),
    queryFn: () => iam.roles(params),
    enabled: isAuthed(),
    ...opts,
  });
}

/** The user directory, filtered by a free-text query. */
export function useUsers(q?: string, opts?: QueryOpts<UserSummary[]>) {
  return useQuery({
    queryKey: queryKeys.users(q),
    queryFn: () => iam.users({ q }),
    enabled: isAuthed(),
    ...opts,
  });
}

/** A single user's full detail record. */
export function useUser(id: string, opts?: QueryOpts<UserDetail>) {
  return useQuery({
    queryKey: queryKeys.user(id),
    queryFn: () => iam.user(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Recent audit-log entries, optionally filtered by limit / action. */
export function useAudit(
  params?: { limit?: number; action?: string },
  opts?: QueryOpts<AuditEntry[]>
) {
  return useQuery({
    queryKey: queryKeys.audit(params),
    queryFn: () => iam.audit(params ?? {}),
    enabled: isAuthed(),
    ...opts,
  });
}

/** The tenant-scoped member directory (client-admin view). */
export function useMembers(opts?: QueryOpts<Member[]>) {
  return useQuery({
    queryKey: queryKeys.members,
    queryFn: () => iam.members(),
    enabled: isAuthed(),
    ...opts,
  });
}

// ---- IAM: mutations ----------------------------------------------------------

/** Toast helper shared by IAM mutations. */
function notifyError(fallback: string) {
  return (e: Error) => toast.error(fallback, { description: e.message });
}

/** Create a user; refreshes the directory. */
export function useCreateUser() {
  const qc = useQueryClient();
  return useMutation<UserDetail, Error, CreateUserInput>({
    mutationFn: (body) => iam.createUser(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["iam", "users"] });
      toast.success("User created");
    },
    onError: notifyError("Couldn't create user"),
  });
}

/** Patch a user's identity fields; refreshes the detail + directory. */
export function useUpdateUser(id: string) {
  const qc = useQueryClient();
  return useMutation<UserDetail, Error, UpdateUserInput>({
    mutationFn: (body) => iam.updateUser(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.user(id) });
      qc.invalidateQueries({ queryKey: ["iam", "users"] });
      toast.success("User updated");
    },
    onError: notifyError("Couldn't update user"),
  });
}

/** Replace a user's profile; refreshes the detail record. */
export function usePutProfile(id: string) {
  const qc = useQueryClient();
  return useMutation<ProfileDto, Error, ProfileInput>({
    mutationFn: (body) => iam.putProfile(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.user(id) });
      toast.success("Profile saved");
    },
    onError: notifyError("Couldn't save profile"),
  });
}

/** Create a role; refreshes the roles list. */
export function useCreateRole() {
  const qc = useQueryClient();
  return useMutation<Role, Error, CreateRoleInput>({
    mutationFn: (body) => iam.createRole(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["iam", "roles"] });
      toast.success("Role created");
    },
    onError: notifyError("Couldn't create role"),
  });
}

/** Update a role (name/description/permissions); refreshes the roles list. */
export function useUpdateRole() {
  const qc = useQueryClient();
  return useMutation<Role, Error, { id: string; body: UpdateRoleInput }>({
    mutationFn: ({ id, body }) => iam.updateRole(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["iam", "roles"] });
      toast.success("Role saved");
    },
    onError: notifyError("Couldn't save role"),
  });
}

/** Delete a custom role; refreshes the roles list. */
export function useDeleteRole() {
  const qc = useQueryClient();
  return useMutation<{ deleted: true }, Error, string>({
    mutationFn: (id) => iam.deleteRole(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["iam", "roles"] });
      toast.success("Role deleted");
    },
    onError: notifyError("Couldn't delete role"),
  });
}

/** Add a membership/persona to a user; refreshes their detail record. */
export function useAddMembership(userId: string) {
  const qc = useQueryClient();
  return useMutation<Membership, Error, MembershipInput>({
    mutationFn: (body) => iam.addMembership(userId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.user(userId) });
      toast.success("Persona added");
    },
    onError: notifyError("Couldn't add persona"),
  });
}

/** Remove a membership; refreshes the owning user's detail record. */
export function useRemoveMembership(userId: string) {
  const qc = useQueryClient();
  return useMutation<void, Error, string>({
    mutationFn: (membershipId) => iam.removeMembership(membershipId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.user(userId) });
      toast.success("Persona removed");
    },
    onError: notifyError("Couldn't remove persona"),
  });
}

/** Assign a role to a user; refreshes their detail record. */
export function useAssignRole(userId: string) {
  const qc = useQueryClient();
  return useMutation<void, Error, { role_id: string; tenant_id?: string }>({
    mutationFn: (body) => iam.assignRole(userId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.user(userId) });
      toast.success("Role assigned");
    },
    onError: notifyError("Couldn't assign role"),
  });
}

/** Revoke a role assignment; refreshes the owning user's detail record. */
export function useRevokeRole(userId: string) {
  const qc = useQueryClient();
  return useMutation<void, Error, string>({
    mutationFn: (userRoleId) => iam.revokeRole(userRoleId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.user(userId) });
      toast.success("Role revoked");
    },
    onError: notifyError("Couldn't revoke role"),
  });
}

/** Invite a tenant-scoped member; refreshes the member directory. */
export function useInviteMember() {
  const qc = useQueryClient();
  return useMutation<Member, Error, InviteMemberInput>({
    mutationFn: (body) => iam.inviteMember(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.members });
      toast.success("Member invited");
    },
    onError: notifyError("Couldn't invite member"),
  });
}

// ---- Property detail + intelligence -----------------------------------------

/** A property's full profile (financials, KPIs, cost breakdown). */
export function useProperty(id: string, opts?: QueryOpts<PropertyProfile>) {
  return useQuery({
    queryKey: queryKeys.property(id),
    queryFn: () => api.property(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Enriched intelligence (detail, valuations, taxes, schools, utilities). */
export function usePropertyIntel(id: string, opts?: QueryOpts<PropertyIntel>) {
  return useQuery({
    queryKey: queryKeys.propertyIntel(id),
    queryFn: () => api.propertyIntel(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** The log of enrichment runs for a property. */
export function usePropertyEnrichment(
  id: string,
  opts?: QueryOpts<EnrichmentRun[]>
) {
  return useQuery({
    queryKey: queryKeys.propertyEnrichment(id),
    queryFn: () => api.propertyEnrichment(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Kick off enrichment; refreshes both the intel and the run log. */
export function useEnrichProperty(id: string) {
  const qc = useQueryClient();
  return useMutation<EnrichResponse, Error, string[] | void>({
    mutationFn: (sources) => api.enrichProperty(id, sources ?? []),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.propertyIntel(id) });
      qc.invalidateQueries({ queryKey: queryKeys.propertyEnrichment(id) });
      toast.success("Enrichment started");
    },
    onError: notifyError("Couldn't start enrichment"),
  });
}

// ---- Investment workflow ----------------------------------------------------

/** The strategy workflow (stages + history) for a property. */
export function useWorkflow(propertyId: string, opts?: QueryOpts<Workflow>) {
  return useQuery({
    queryKey: queryKeys.workflow(propertyId),
    queryFn: () => api.workflow(propertyId),
    enabled: isAuthed() && !!propertyId,
    ...opts,
  });
}

/** Advance the workflow to a stage; refreshes the workflow + property. */
export function useAdvanceWorkflow(propertyId: string) {
  const qc = useQueryClient();
  return useMutation<Workflow, Error, { to_stage: string; note?: string }>({
    mutationFn: ({ to_stage, note }) =>
      api.advanceWorkflow(propertyId, to_stage, note),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.workflow(propertyId) });
      qc.invalidateQueries({ queryKey: queryKeys.property(propertyId) });
      toast.success("Workflow advanced");
    },
    onError: notifyError("Couldn't advance workflow"),
  });
}

// ---- Rentals: units ---------------------------------------------------------

/** Units belonging to a property. */
export function useUnits(propertyId: string, opts?: QueryOpts<Unit[]>) {
  return useQuery({
    queryKey: queryKeys.units(propertyId),
    queryFn: () => api.units(propertyId),
    enabled: isAuthed() && !!propertyId,
    ...opts,
  });
}

/** Create a unit; refreshes the property's unit list. */
export function useCreateUnit(propertyId: string) {
  const qc = useQueryClient();
  return useMutation<Unit, Error, CreateUnitInput>({
    mutationFn: (body) => api.createUnit(propertyId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.units(propertyId) });
      toast.success("Unit added");
    },
    onError: notifyError("Couldn't add unit"),
  });
}

// ---- Rentals: leases --------------------------------------------------------

/** Leases across the portfolio, optionally filtered by status / property. */
export function useLeases(
  params?: { status?: string; property_id?: string },
  opts?: QueryOpts<Lease[]>
) {
  return useQuery({
    queryKey: queryKeys.leases(params),
    queryFn: () => api.leases(params ?? {}),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Leases scoped to a single property. */
export function usePropertyLeases(
  propertyId: string,
  opts?: QueryOpts<Lease[]>
) {
  return useQuery({
    queryKey: queryKeys.propertyLeases(propertyId),
    queryFn: () => api.propertyLeases(propertyId),
    enabled: isAuthed() && !!propertyId,
    ...opts,
  });
}

/** A single lease with its payment schedule. */
export function useLease(id: string, opts?: QueryOpts<LeaseDetail>) {
  return useQuery({
    queryKey: queryKeys.lease(id),
    queryFn: () => api.lease(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Create a lease; refreshes the property + portfolio lease lists. */
export function useCreateLease(propertyId: string) {
  const qc = useQueryClient();
  return useMutation<Lease, Error, CreateLeaseInput>({
    mutationFn: (body) => api.createLease(propertyId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.propertyLeases(propertyId) });
      qc.invalidateQueries({ queryKey: ["leases"] });
      toast.success("Lease created");
    },
    onError: notifyError("Couldn't create lease"),
  });
}

/** Record a payment against a lease; refreshes the lease detail + lists. */
export function useRecordPayment(leaseId: string) {
  const qc = useQueryClient();
  return useMutation<LeasePayment, Error, RecordPaymentInput>({
    mutationFn: (body) => api.recordPayment(leaseId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.lease(leaseId) });
      qc.invalidateQueries({ queryKey: ["leases"] });
      toast.success("Payment recorded");
    },
    onError: notifyError("Couldn't record payment"),
  });
}

// ---- Maintenance: tickets ---------------------------------------------------

/** Tickets across the portfolio, optionally filtered. */
export function useTickets(
  params?: { status?: string; property_id?: string; priority?: string },
  opts?: QueryOpts<MaintenanceTicket[]>
) {
  return useQuery({
    queryKey: queryKeys.tickets(params),
    queryFn: () => api.tickets(params ?? {}),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Tickets scoped to a single property. */
export function usePropertyTickets(
  propertyId: string,
  opts?: QueryOpts<MaintenanceTicket[]>
) {
  return useQuery({
    queryKey: queryKeys.propertyTickets(propertyId),
    queryFn: () => api.propertyTickets(propertyId),
    enabled: isAuthed() && !!propertyId,
    ...opts,
  });
}

/** A single ticket with its comment thread. */
export function useTicket(id: string, opts?: QueryOpts<TicketDetail>) {
  return useQuery({
    queryKey: queryKeys.ticket(id),
    queryFn: () => api.ticket(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Create a ticket; refreshes the property + portfolio ticket lists. */
export function useCreateTicket(propertyId: string) {
  const qc = useQueryClient();
  return useMutation<MaintenanceTicket, Error, CreateTicketInput>({
    mutationFn: (body) => api.createTicket(propertyId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.propertyTickets(propertyId) });
      qc.invalidateQueries({ queryKey: ["tickets"] });
      toast.success("Ticket created");
    },
    onError: notifyError("Couldn't create ticket"),
  });
}

/** Patch a ticket; refreshes the ticket detail + lists. */
export function useUpdateTicket(id: string) {
  const qc = useQueryClient();
  return useMutation<MaintenanceTicket, Error, UpdateTicketInput>({
    mutationFn: (body) => api.updateTicket(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.ticket(id) });
      qc.invalidateQueries({ queryKey: ["tickets"] });
      qc.invalidateQueries({ queryKey: ["properties"] });
      toast.success("Ticket updated");
    },
    onError: notifyError("Couldn't update ticket"),
  });
}

/** Add a comment to a ticket; refreshes the ticket detail. */
export function useAddTicketComment(id: string) {
  const qc = useQueryClient();
  return useMutation<unknown, Error, string>({
    mutationFn: (body) => api.addTicketComment(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.ticket(id) });
      toast.success("Comment added");
    },
    onError: notifyError("Couldn't add comment"),
  });
}

// ---- Title: ownership + liens -----------------------------------------------

/** Ownership records (vesting / deed) for a property. */
export function useOwnership(propertyId: string, opts?: QueryOpts<Ownership[]>) {
  return useQuery({
    queryKey: queryKeys.ownership(propertyId),
    queryFn: () => api.ownership(propertyId),
    enabled: isAuthed() && !!propertyId,
    ...opts,
  });
}

/** Add an ownership record; refreshes the property's ownership list. */
export function useCreateOwnership(propertyId: string) {
  const qc = useQueryClient();
  return useMutation<Ownership, Error, CreateOwnershipInput>({
    mutationFn: (body) => api.createOwnership(propertyId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.ownership(propertyId) });
      toast.success("Ownership recorded");
    },
    onError: notifyError("Couldn't record ownership"),
  });
}

/** Liens recorded against a property. */
export function useLiens(propertyId: string, opts?: QueryOpts<Lien[]>) {
  return useQuery({
    queryKey: queryKeys.liens(propertyId),
    queryFn: () => api.liens(propertyId),
    enabled: isAuthed() && !!propertyId,
    ...opts,
  });
}

/** Add a lien; refreshes the property's lien list. */
export function useCreateLien(propertyId: string) {
  const qc = useQueryClient();
  return useMutation<Lien, Error, CreateLienInput>({
    mutationFn: (body) => api.createLien(propertyId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.liens(propertyId) });
      toast.success("Lien recorded");
    },
    onError: notifyError("Couldn't record lien"),
  });
}

// ---- Financing: mortgages ---------------------------------------------------

/** Mortgages financing a property. */
export function useMortgages(propertyId: string, opts?: QueryOpts<Mortgage[]>) {
  return useQuery({
    queryKey: queryKeys.mortgages(propertyId),
    queryFn: () => api.mortgages(propertyId),
    enabled: isAuthed() && !!propertyId,
    ...opts,
  });
}

/** Add a mortgage; refreshes the mortgage list + property financials. */
export function useCreateMortgage(propertyId: string) {
  const qc = useQueryClient();
  return useMutation<Mortgage, Error, CreateMortgageInput>({
    mutationFn: (body) => api.createMortgage(propertyId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.mortgages(propertyId) });
      qc.invalidateQueries({ queryKey: queryKeys.property(propertyId) });
      toast.success("Mortgage added");
    },
    onError: notifyError("Couldn't add mortgage"),
  });
}

/** Delete a mortgage; refreshes mortgage lists + property financials. */
export function useDeleteMortgage() {
  const qc = useQueryClient();
  return useMutation<void, Error, string>({
    mutationFn: (id) => api.deleteMortgage(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["properties"] });
      toast.success("Mortgage deleted");
    },
    onError: notifyError("Couldn't delete mortgage"),
  });
}

// ---- Entities (counterparties) ----------------------------------------------

/** The counterparty registry, optionally filtered by kind. */
export function useEntities(kind?: string, opts?: QueryOpts<Counterparty[]>) {
  return useQuery({
    queryKey: queryKeys.entities(kind),
    queryFn: () => api.entities(kind),
    enabled: isAuthed(),
    ...opts,
  });
}

/** A single counterparty with its note log. */
export function useEntity(id: string, opts?: QueryOpts<CounterpartyDetail>) {
  return useQuery({
    queryKey: queryKeys.entity(id),
    queryFn: () => api.entity(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Create a counterparty; refreshes the registry. */
export function useCreateEntity() {
  const qc = useQueryClient();
  return useMutation<Counterparty, Error, CreateCounterpartyInput>({
    mutationFn: (body) => api.createEntity(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["entities"] });
      toast.success("Entity created");
    },
    onError: notifyError("Couldn't create entity"),
  });
}

/** Append a note to a counterparty; refreshes its detail record. */
export function useAddEntityNote(id: string) {
  const qc = useQueryClient();
  return useMutation<CounterpartyNote, Error, string>({
    mutationFn: (body) => api.addEntityNote(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.entity(id) });
      toast.success("Note added");
    },
    onError: notifyError("Couldn't add note"),
  });
}

// ---- LLCs -------------------------------------------------------------------

/** LLC groups (ownership rollups across the portfolio). */
export function useLlcGroups(opts?: QueryOpts<LlcGroup[]>) {
  return useQuery({
    queryKey: queryKeys.llcGroups,
    queryFn: () => api.llcGroups(),
    enabled: isAuthed(),
    ...opts,
  });
}

/** A single LLC's onboarding profile. */
export function useLlc(id: string, opts?: QueryOpts<Llc>) {
  return useQuery({
    queryKey: queryKeys.llc(id),
    queryFn: () => api.llc(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Create an LLC; refreshes the LLC groups rollup. */
export function useCreateLlc() {
  const qc = useQueryClient();
  return useMutation<
    Llc,
    Error,
    { name: string; ein?: string; state?: string; entity_type?: string }
  >({
    mutationFn: (body) => api.createLlc(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.llcGroups });
      toast.success("LLC created");
    },
    onError: notifyError("Couldn't create LLC"),
  });
}

/** Patch an LLC profile; refreshes the LLC + groups rollup. */
export function useUpdateLlc(id: string) {
  const qc = useQueryClient();
  return useMutation<Llc, Error, UpdateLlcInput>({
    mutationFn: (body) => api.updateLlc(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.llc(id) });
      qc.invalidateQueries({ queryKey: queryKeys.llcGroups });
      toast.success("LLC updated");
    },
    onError: notifyError("Couldn't update LLC"),
  });
}

/** Documents uploaded for an LLC. */
export function useLlcDocuments(id: string, opts?: QueryOpts<LlcDocument[]>) {
  return useQuery({
    queryKey: queryKeys.llcDocuments(id),
    queryFn: () => api.llcDocuments(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Upload an LLC document; refreshes the document list. */
export function useUploadLlcDocument(id: string) {
  const qc = useQueryClient();
  return useMutation<
    LlcDocument,
    Error,
    { file: File; kind: string; title?: string }
  >({
    mutationFn: ({ file, kind, title }) =>
      api.uploadLlcDocument(id, file, kind, title),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.llcDocuments(id) });
      toast.success("Document uploaded");
    },
    onError: notifyError("Couldn't upload document"),
  });
}

/** Delete an LLC document; refreshes the document list. */
export function useDeleteLlcDocument(id: string) {
  const qc = useQueryClient();
  return useMutation<{ deleted: boolean }, Error, string>({
    mutationFn: (docId) => api.deleteLlcDocument(id, docId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.llcDocuments(id) });
      toast.success("Document deleted");
    },
    onError: notifyError("Couldn't delete document"),
  });
}

/** Branding (logo, colors, signature block) for an LLC. */
export function useLlcBranding(id: string, opts?: QueryOpts<LlcBranding>) {
  return useQuery({
    queryKey: queryKeys.llcBranding(id),
    queryFn: () => api.llcBranding(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Replace an LLC's branding; refreshes the branding record. */
export function usePutLlcBranding(id: string) {
  const qc = useQueryClient();
  return useMutation<LlcBranding, Error, Omit<LlcBranding, "llc_id">>({
    mutationFn: (body) => api.putLlcBranding(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.llcBranding(id) });
      toast.success("Branding saved");
    },
    onError: notifyError("Couldn't save branding"),
  });
}

/** Document templates configured for an LLC. */
export function useLlcTemplates(id: string, opts?: QueryOpts<LlcTemplate[]>) {
  return useQuery({
    queryKey: queryKeys.llcTemplates(id),
    queryFn: () => api.llcTemplates(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Create a template; refreshes the template list. */
export function useCreateLlcTemplate(id: string) {
  const qc = useQueryClient();
  return useMutation<LlcTemplate, Error, CreateTemplateInput>({
    mutationFn: (body) => api.createLlcTemplate(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.llcTemplates(id) });
      toast.success("Template created");
    },
    onError: notifyError("Couldn't create template"),
  });
}

/** Patch a template; refreshes the template list. */
export function useUpdateLlcTemplate(id: string) {
  const qc = useQueryClient();
  return useMutation<
    LlcTemplate,
    Error,
    { templateId: string; body: Partial<CreateTemplateInput> }
  >({
    mutationFn: ({ templateId, body }) =>
      api.updateLlcTemplate(id, templateId, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.llcTemplates(id) });
      toast.success("Template saved");
    },
    onError: notifyError("Couldn't save template"),
  });
}

/** Delete a template; refreshes the template list. */
export function useDeleteLlcTemplate(id: string) {
  const qc = useQueryClient();
  return useMutation<{ deleted: boolean }, Error, string>({
    mutationFn: (templateId) => api.deleteLlcTemplate(id, templateId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.llcTemplates(id) });
      toast.success("Template deleted");
    },
    onError: notifyError("Couldn't delete template"),
  });
}

/** Generated documents (rendered + optionally emailed) for an LLC. */
export function useGeneratedDocuments(
  id: string,
  opts?: QueryOpts<GeneratedDocument[]>
) {
  return useQuery({
    queryKey: queryKeys.generatedDocuments(id),
    queryFn: () => api.generatedDocuments(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Generate a document from a template; refreshes the generated list. */
export function useGenerateDocument(id: string) {
  const qc = useQueryClient();
  return useMutation<GeneratedDocument, Error, GenerateDocumentInput>({
    mutationFn: (body) => api.generateDocument(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.generatedDocuments(id) });
      toast.success("Document generated");
    },
    onError: notifyError("Couldn't generate document"),
  });
}

// ---- Storage configuration --------------------------------------------------

/** The tenant's storage backend configuration. */
export function useStorageConfig(opts?: QueryOpts<StorageConfig>) {
  return useQuery({
    queryKey: queryKeys.storageConfig,
    queryFn: () => api.storageConfig(),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Replace the storage configuration; refreshes the config. */
export function usePutStorageConfig() {
  const qc = useQueryClient();
  return useMutation<StorageConfig, Error, UpdateStorageConfigInput>({
    mutationFn: (body) => api.putStorageConfig(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.storageConfig });
      toast.success("Storage configuration saved");
    },
    onError: notifyError("Couldn't save storage configuration"),
  });
}

// ---- Flips module (preview) -------------------------------------------------

/** The flips deal pipeline (preview module). */
export function useFlipPipeline(opts?: QueryOpts<FlipPipeline>) {
  return useQuery({
    queryKey: queryKeys.flipPipeline,
    queryFn: () => api.flipPipeline(),
    enabled: isAuthed(),
    ...opts,
  });
}

// ---- Platform (staff) -------------------------------------------------------

/** All tenants on the platform (staff view). */
export function usePlatformTenants(opts?: QueryOpts<TenantSummary[]>) {
  return useQuery({
    queryKey: queryKeys.platformTenants,
    queryFn: () => api.platformTenants(),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Aggregate platform metrics (staff view). */
export function usePlatformMetrics(opts?: QueryOpts<PlatformMetrics>) {
  return useQuery({
    queryKey: queryKeys.platformMetrics,
    queryFn: () => api.platformMetrics(),
    enabled: isAuthed(),
    ...opts,
  });
}

/** A single tenant's detail + rollups (staff view). */
export function usePlatformTenant(id: string, opts?: QueryOpts<TenantDetail>) {
  return useQuery({
    queryKey: queryKeys.platformTenant(id),
    queryFn: () => api.platformTenant(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Provision a new tenant; refreshes the tenant directory + metrics. */
export function useCreateTenant() {
  const qc = useQueryClient();
  return useMutation<TenantSummary, Error, CreateTenantInput>({
    mutationFn: (body) => api.createTenant(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.platformTenants });
      qc.invalidateQueries({ queryKey: queryKeys.platformMetrics });
      toast.success("Tenant created");
    },
    onError: notifyError("Couldn't create tenant"),
  });
}

/** Update a tenant (status/plan/name/domain); refreshes directory + detail. */
export function useUpdateTenant() {
  const qc = useQueryClient();
  return useMutation<
    TenantSummary,
    Error,
    { id: string; body: UpdateTenantInput }
  >({
    mutationFn: ({ id, body }) => api.updateTenant(id, body),
    onSuccess: (_data, { id }) => {
      qc.invalidateQueries({ queryKey: queryKeys.platformTenants });
      qc.invalidateQueries({ queryKey: queryKeys.platformTenant(id) });
      qc.invalidateQueries({ queryKey: queryKeys.platformMetrics });
      toast.success("Tenant updated");
    },
    onError: notifyError("Couldn't update tenant"),
  });
}

// ---- Tenant theme / white-label branding ------------------------------------

/** The active tenant's full theme (branding editor). */
export function useTenantTheme(opts?: QueryOpts<Theme>) {
  return useQuery({
    queryKey: queryKeys.tenantTheme,
    queryFn: () => api.theme(),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Persist branding/colour changes; refreshes the theme. */
export function useUpdateTenantTheme() {
  const qc = useQueryClient();
  return useMutation<Theme, Error, UpdateThemeInput>({
    mutationFn: (body) => api.updateTheme(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.tenantTheme });
      toast.success("Branding saved");
    },
    onError: notifyError("Couldn't save branding"),
  });
}

// ---- Modules ----------------------------------------------------------------

/** Toggle a module's enablement for the active tenant; refreshes modules. */
export function useSetModule() {
  const qc = useQueryClient();
  return useMutation<ModuleInfo, Error, { key: string; enabled: boolean }>({
    mutationFn: ({ key, enabled }) => api.setModule(key, enabled),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.modules });
      toast.success("Module updated");
    },
    onError: notifyError("Couldn't update module"),
  });
}
