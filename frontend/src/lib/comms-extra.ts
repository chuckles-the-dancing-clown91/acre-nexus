// API calls the communication and people pages need that `lib/api.ts` and
// `lib/backoffice.ts` don't have yet. Same client, same auth.

import { request } from "./api";

export interface OwnerInviteResult {
  owner_id: string;
  user_id: string;
  /** `invited`: a set-password link went out. `linked`: they already had an
   * account, which now opens the owner portal. */
  outcome: "invited" | "linked";
}

export interface OwnerLimitResult {
  id: string;
  approval_limit_cents: number | null;
  user_id: string | null;
}

export const ownersExtra = {
  /** Give an owner a login to the owner portal (`entity:manage` + `member:manage`). */
  invite: (ownerId: string) =>
    request<OwnerInviteResult>(`/crm/owners/${ownerId}/invite`, {
      method: "POST",
      auth: true,
    }),
  /** An owner's own approval limit, or back to the workspace's with `null`. */
  setApprovalLimit: (ownerId: string, cents: number | null) =>
    request<OwnerLimitResult>(`/crm/owners/${ownerId}/approvals`, {
      method: "PATCH",
      auth: true,
      body:
        cents === null
          ? { clear_limit: true }
          : { approval_limit_cents: cents },
    }),
};
