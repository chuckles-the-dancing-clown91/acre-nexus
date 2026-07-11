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
  type BankAccount,
  type BankTxn,
  type ComputePayoutInput,
  type CreateReminderInput,
  type CreateRoleInput,
  type CreateTokenResponse,
  type CreateUserInput,
  type CreateVendorBillInput,
  type DomainInfo,
  type FinanceSeries,
  type InviteMemberInput,
  type Lead,
  type LeadsResponse,
  type LedgerAccount,
  type LedgerTxn,
  type LegalEntity,
  type ManualTxnInput,
  type Member,
  type MyLease,
  type Membership,
  type MembershipInput,
  type ModuleInfo,
  type Payment,
  type Payout,
  type PermissionDef,
  type ProfileDto,
  type ProfileInput,
  type ProfileType,
  type Reminder,
  type Role,
  type TokenSummary,
  type TrialBalance,
  type TrustReconciliation,
  type UpdateLeadInput,
  type UpdateReminderInput,
  type UpdateRoleInput,
  type UpdateUserInput,
  type UserDetail,
  type UserSummary,
  type VendorBill,
} from "./api";
import { tokenStore } from "./api";
import { toast } from "sonner";
import type {
  Application,
  Assignment,
  AssignmentSubject,
  CreateAssignmentInput,
  PortfolioSummary,
  Property,
  SettingView,
} from "./types";

/** Centralised, hierarchical query keys. */
export const queryKeys = {
  modules: ["modules"] as const,
  properties: ["properties"] as const,
  property: (id: string) => ["properties", id] as const,
  portfolioSummary: ["portfolio", "summary"] as const,
  applications: ["applications"] as const,
  apiTokens: ["api-tokens"] as const,
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
  assignments: (subjectType: AssignmentSubject, id: string) =>
    ["assignments", subjectType, id] as const,
  settings: ["settings"] as const,
  // Accounting & payments (Phase 3)
  llcs: ["llcs"] as const,
  llcGroups: ["portfolio", "llcs"] as const,
  ledgerAccounts: (entityId: string) =>
    ["accounting", "accounts", entityId] as const,
  ledgerTransactions: (entityId: string) =>
    ["accounting", "transactions", entityId] as const,
  trialBalance: (entityId: string) =>
    ["accounting", "trial-balance", entityId] as const,
  trustReconciliation: (entityId: string) =>
    ["accounting", "trust", entityId] as const,
  financeSeries: (months: number) => ["finance", "series", months] as const,
  payments: (params?: { status?: string; lease?: string }) =>
    ["payments", params ?? {}] as const,
  myLease: ["my", "lease"] as const,
  bankAccounts: (entityId?: string) =>
    ["bank-accounts", entityId ?? "all"] as const,
  bankTransactions: (accountId: string) =>
    ["bank-transactions", accountId] as const,
  payouts: ["payouts"] as const,
  // Accounts payable (#58)
  payables: (params?: { status?: string }) =>
    ["payables", params ?? {}] as const,
  // Calendar / reminders (#54)
  reminders: (params?: {
    from?: string;
    to?: string;
    subject_type?: string;
    status?: string;
  }) => ["reminders", params ?? {}] as const,
  // CRM leads (#62)
  leads: (status?: string) => ["leads", status ?? ""] as const,
  // White-label domains
  domains: ["domains"] as const,
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

// ---- White-label domains -----------------------------------------------------

export function useDomains(opts?: QueryOpts<DomainInfo[]>) {
  return useQuery({
    queryKey: queryKeys.domains,
    queryFn: () => api.domains(),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Mutations below all invalidate the domain list on success. */
function useDomainMutation<TArgs>(fn: (args: TArgs) => Promise<unknown>) {
  const qc = useQueryClient();
  return useMutation<unknown, Error, TArgs>({
    mutationFn: fn,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.domains });
    },
  });
}

export function useCreateDomain() {
  return useDomainMutation<{ hostname: string; audience: string }>(
    ({ hostname, audience }) => api.createDomain(hostname, audience)
  );
}

export function useVerifyDomain() {
  return useDomainMutation<string>((id) => api.verifyDomain(id));
}

export function useDeleteDomain() {
  return useDomainMutation<string>((id) => api.deleteDomain(id));
}

export function useVerifyDomainEmail() {
  return useDomainMutation<string>((id) => api.verifyDomainEmail(id));
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

// ---- Staff assignments (property / LLC) ------------------------------------

/** The assigned team for a property or legal entity. */
export function useAssignments(
  subjectType: AssignmentSubject,
  id: string,
  opts?: QueryOpts<Assignment[]>
) {
  return useQuery({
    queryKey: queryKeys.assignments(subjectType, id),
    queryFn: () =>
      subjectType === "entity"
        ? api.entityAssignments(id)
        : api.propertyAssignments(id),
    enabled: isAuthed() && !!id,
    ...opts,
  });
}

/** Assign a person (also grants scoped access); refreshes the team + subject. */
export function useCreateAssignment(
  subjectType: AssignmentSubject,
  id: string
) {
  const qc = useQueryClient();
  return useMutation<Assignment, Error, CreateAssignmentInput>({
    mutationFn: (body) =>
      subjectType === "entity"
        ? api.createEntityAssignment(id, body)
        : api.createPropertyAssignment(id, body),
    onSuccess: () => {
      qc.invalidateQueries({
        queryKey: queryKeys.assignments(subjectType, id),
      });
      if (subjectType === "property") {
        qc.invalidateQueries({ queryKey: queryKeys.property(id) });
      }
      toast.success("Assigned");
    },
    onError: notifyError("Couldn't assign"),
  });
}

/** Unassign a person (revokes their scoped grant); refreshes the team. */
export function useDeleteAssignment(
  subjectType: AssignmentSubject,
  id: string
) {
  const qc = useQueryClient();
  return useMutation<{ removed: boolean }, Error, string>({
    mutationFn: (assignmentId) =>
      subjectType === "entity"
        ? api.deleteEntityAssignment(id, assignmentId)
        : api.deletePropertyAssignment(id, assignmentId),
    onSuccess: () => {
      qc.invalidateQueries({
        queryKey: queryKeys.assignments(subjectType, id),
      });
      if (subjectType === "property") {
        qc.invalidateQueries({ queryKey: queryKeys.property(id) });
      }
      toast.success("Removed");
    },
    onError: notifyError("Couldn't remove assignment"),
  });
}

// ---- System settings -------------------------------------------------------

/** The per-tenant settings catalog with effective values. */
export function useSettings(opts?: QueryOpts<SettingView[]>) {
  return useQuery({
    queryKey: queryKeys.settings,
    queryFn: () => api.settings(),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Override one setting; refreshes the settings list. */
export function useSetSetting() {
  const qc = useQueryClient();
  return useMutation<SettingView, Error, { key: string; value: unknown }>({
    mutationFn: ({ key, value }) => api.setSetting(key, value),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.settings });
      toast.success("Setting saved");
    },
    onError: notifyError("Couldn't save setting"),
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

// ---------------------------------------------------------------------------
// Accounting & payments (Phase 3)
// ---------------------------------------------------------------------------

export function useLegalEntities(opts?: QueryOpts<LegalEntity[]>) {
  return useQuery({
    queryKey: queryKeys.llcs,
    queryFn: () => api.legalEntities(),
    enabled: isAuthed(),
    ...opts,
  });
}

/**
 * Entity picker state shared by the accounting/payouts pages: all legal
 * entities, defaulting to the one holding the most properties (its books are
 * the interesting ones).
 */
export function useEntityPicker() {
  const { data: entities } = useLegalEntities();
  const { data: groups } = useQuery({
    queryKey: queryKeys.llcGroups,
    queryFn: () => api.llcGroups(),
    enabled: isAuthed(),
  });
  const biggest =
    groups && groups.length > 0
      ? groups.reduce((a, b) => (b.property_count > a.property_count ? b : a))
      : undefined;
  const defaultId = biggest?.id ?? entities?.[0]?.id;
  return { entities, defaultId };
}

export function useLedgerAccounts(
  entityId: string | undefined,
  opts?: QueryOpts<LedgerAccount[]>
) {
  return useQuery({
    queryKey: queryKeys.ledgerAccounts(entityId ?? ""),
    queryFn: () => api.ledgerAccounts(entityId!),
    enabled: isAuthed() && !!entityId,
    ...opts,
  });
}

export function useLedgerTransactions(
  entityId: string | undefined,
  opts?: QueryOpts<LedgerTxn[]>
) {
  return useQuery({
    queryKey: queryKeys.ledgerTransactions(entityId ?? ""),
    queryFn: () => api.ledgerTransactions(entityId!),
    enabled: isAuthed() && !!entityId,
    ...opts,
  });
}

export function useTrialBalance(
  entityId: string | undefined,
  opts?: QueryOpts<TrialBalance>
) {
  return useQuery({
    queryKey: queryKeys.trialBalance(entityId ?? ""),
    queryFn: () => api.trialBalance(entityId!),
    enabled: isAuthed() && !!entityId,
    ...opts,
  });
}

export function useTrustReconciliation(
  entityId: string | undefined,
  opts?: QueryOpts<TrustReconciliation>
) {
  return useQuery({
    queryKey: queryKeys.trustReconciliation(entityId ?? ""),
    queryFn: () => api.trustReconciliation(entityId!),
    enabled: isAuthed() && !!entityId,
    ...opts,
  });
}

export function useFinanceSeries(months = 12, opts?: QueryOpts<FinanceSeries>) {
  return useQuery({
    queryKey: queryKeys.financeSeries(months),
    queryFn: () => api.financeSeries(months),
    enabled: isAuthed(),
    ...opts,
  });
}

export function usePayments(
  params: { status?: string; lease?: string } = {},
  opts?: QueryOpts<Payment[]>
) {
  return useQuery({
    queryKey: queryKeys.payments(params),
    queryFn: () => api.payments(params),
    enabled: isAuthed(),
    ...opts,
  });
}

/** Post a manual journal entry; refreshes the entity's books. */
export function usePostLedgerTransaction(entityId: string) {
  const qc = useQueryClient();
  return useMutation<{ id: string }, Error, ManualTxnInput>({
    mutationFn: (body) => api.postLedgerTransaction(body),
    onSuccess: () => {
      qc.invalidateQueries({
        queryKey: queryKeys.ledgerTransactions(entityId),
      });
      qc.invalidateQueries({ queryKey: queryKeys.ledgerAccounts(entityId) });
      qc.invalidateQueries({ queryKey: queryKeys.trialBalance(entityId) });
      toast.success("Journal entry posted");
    },
    onError: notifyError("Couldn't post the entry"),
  });
}

export function useBankAccounts(
  entityId?: string,
  opts?: QueryOpts<BankAccount[]>
) {
  return useQuery({
    queryKey: queryKeys.bankAccounts(entityId),
    queryFn: () => api.allBankAccounts(entityId),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useBankTransactions(
  accountId: string | undefined,
  opts?: QueryOpts<BankTxn[]>
) {
  return useQuery({
    queryKey: queryKeys.bankTransactions(accountId ?? ""),
    queryFn: () => api.bankTransactions(accountId!),
    enabled: isAuthed() && !!accountId,
    ...opts,
  });
}

export function useMyLease(opts?: QueryOpts<MyLease>) {
  return useQuery({
    queryKey: queryKeys.myLease,
    queryFn: () => api.myLease(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function usePayouts(opts?: QueryOpts<Payout[]>) {
  return useQuery({
    queryKey: queryKeys.payouts,
    queryFn: () => api.payouts(),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useComputePayout() {
  const qc = useQueryClient();
  return useMutation<Payout, Error, ComputePayoutInput>({
    mutationFn: (body) => api.computePayout(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.payouts });
      toast.success("Payout computed from the ledger");
    },
    onError: notifyError("Couldn't compute the payout"),
  });
}

export function useExecutePayout() {
  const qc = useQueryClient();
  return useMutation<Payout, Error, string>({
    mutationFn: (id) => api.executePayout(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.payouts });
      toast.success("Payout executing — settlement posts the statement");
    },
    onError: notifyError("Couldn't execute the payout"),
  });
}

// ---- Accounts payable (#58) -------------------------------------------------

export function usePayables(
  params: { status?: string } = {},
  opts?: QueryOpts<VendorBill[]>
) {
  return useQuery({
    queryKey: queryKeys.payables(params),
    queryFn: () => api.payables(params),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useCreatePayable() {
  const qc = useQueryClient();
  return useMutation<VendorBill, Error, CreateVendorBillInput>({
    mutationFn: (body) => api.createPayable(body),
    onSuccess: (bill) => {
      qc.invalidateQueries({ queryKey: ["payables"] });
      toast.success(`Bill ${bill.bill_number} drafted`);
    },
    onError: notifyError("Couldn't create the bill"),
  });
}

/** One hook for the lifecycle actions — submit / approve / reject / void / pay. */
export function usePayableAction() {
  const qc = useQueryClient();
  return useMutation<
    VendorBill,
    Error,
    {
      id: string;
      action: "submit" | "approve" | "reject" | "void" | "pay";
      reason?: string;
    }
  >({
    mutationFn: ({ id, action, reason }) => {
      switch (action) {
        case "submit":
          return api.submitPayable(id);
        case "approve":
          return api.approvePayable(id);
        case "reject":
          return api.rejectPayable(id, reason);
        case "void":
          return api.voidPayable(id);
        case "pay":
          return api.payPayable(id);
      }
    },
    onSuccess: (bill, { action }) => {
      qc.invalidateQueries({ queryKey: ["payables"] });
      const message: Record<string, string> = {
        submit: `Bill ${bill.bill_number} submitted for approval`,
        approve: `Bill ${bill.bill_number} approved — expense accrued to the ledger`,
        reject: `Bill ${bill.bill_number} returned to draft`,
        void: `Bill ${bill.bill_number} voided`,
        pay: `Bill ${bill.bill_number} paying — settlement posts to the ledger`,
      };
      toast.success(message[action]);
    },
    onError: notifyError("Couldn't update the bill"),
  });
}

// ---- Calendar / reminders (#54) ----------------------------------------------

export function useReminders(
  params: {
    from?: string;
    to?: string;
    subject_type?: string;
    status?: string;
  } = {},
  opts?: QueryOpts<Reminder[]>
) {
  return useQuery({
    queryKey: queryKeys.reminders(params),
    queryFn: () => api.reminders(params),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useCreateReminder() {
  const qc = useQueryClient();
  return useMutation<Reminder, Error, CreateReminderInput>({
    mutationFn: (body) => api.createReminder(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["reminders"] });
      toast.success("Reminder scheduled");
    },
    onError: notifyError("Couldn't create the reminder"),
  });
}

export function useUpdateReminder() {
  const qc = useQueryClient();
  return useMutation<
    Reminder,
    Error,
    { id: string; body: UpdateReminderInput }
  >({
    mutationFn: ({ id, body }) => api.updateReminder(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["reminders"] });
      toast.success("Reminder updated");
    },
    onError: notifyError("Couldn't update the reminder"),
  });
}

export function useDeleteReminder() {
  const qc = useQueryClient();
  return useMutation<{ deleted: boolean }, Error, string>({
    mutationFn: (id) => api.deleteReminder(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["reminders"] });
      toast.success("Reminder removed");
    },
    onError: notifyError("Couldn't remove the reminder"),
  });
}

// ---- CRM leads (#62) ----------------------------------------------------------

export function useLeads(status?: string, opts?: QueryOpts<LeadsResponse>) {
  return useQuery({
    queryKey: queryKeys.leads(status),
    queryFn: () => api.leads(status),
    enabled: isAuthed(),
    ...opts,
  });
}

export function useUpdateLead() {
  const qc = useQueryClient();
  return useMutation<Lead, Error, { id: string; body: UpdateLeadInput }>({
    mutationFn: ({ id, body }) => api.updateLead(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["leads"] });
      toast.success("Lead updated");
    },
    onError: notifyError("Couldn't update the lead"),
  });
}
