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
  type InviteMemberInput,
  type Member,
  type Membership,
  type MembershipInput,
  type ModuleInfo,
  type PermissionDef,
  type ProfileDto,
  type ProfileInput,
  type ProfileType,
  type Role,
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
  Assignment,
  AssignmentSubject,
  CreateAssignmentInput,
  PortfolioSummary,
  Property,
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
