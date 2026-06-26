// Pure workspace/persona helpers shared by the console UI and unit tests.

import type { Membership, User, Workspace } from "./types";

/**
 * Resolve the user's currently active workspace from their `workspaces` list
 * and `active_tenant_id`. When `active_tenant_id` is null the platform ("Acre
 * HQ") workspace is active; otherwise the tenant workspace whose `tenant_id`
 * matches. Returns `null` if no workspace matches (e.g. empty list).
 */
export function activeWorkspace(
  user: Pick<User, "active_tenant_id" | "workspaces"> | null | undefined
): Workspace | null {
  if (!user) return null;
  if (user.active_tenant_id === null) {
    return user.workspaces.find((w) => w.kind === "platform") ?? null;
  }
  return (
    user.workspaces.find((w) => w.tenant_id === user.active_tenant_id) ?? null
  );
}

/**
 * Resolve the membership that backs the active workspace: a platform membership
 * when active is null, otherwise the membership matching `active_tenant_id`.
 * Prefers a primary membership when several match. Returns `null` if none.
 */
export function activeMembership(
  user: Pick<User, "active_tenant_id" | "memberships"> | null | undefined
): Membership | null {
  if (!user) return null;
  const matches =
    user.active_tenant_id === null
      ? user.memberships.filter((m) => m.scope === "platform")
      : user.memberships.filter((m) => m.tenant_id === user.active_tenant_id);
  if (matches.length === 0) return null;
  return matches.find((m) => m.is_primary) ?? matches[0];
}
