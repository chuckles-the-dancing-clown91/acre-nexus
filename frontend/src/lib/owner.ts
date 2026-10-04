// The owner portal: what an owner holds, what's waiting on them, the work
// on their properties, and their monthly statement.

import { request } from "@/lib/api";
import type { User } from "@/lib/types";

export interface OwnerApproval {
  id: string;
  ticket_id: string;
  ticket_title: string;
  property_id: string;
  property: string;
  owner_id: string;
  owner_name: string;
  /** approval | signoff */
  kind: string;
  amount_cents: number;
  amount_label: string;
  /** pending | approved | declined | disputed | overridden */
  status: string;
  note: string | null;
  requested_at: string;
  decided_at: string | null;
  decided_by: string | null;
  decision_note: string | null;
  override_reason: string | null;
  nudges: number;
}

export interface OwnerProperty {
  id: string;
  name: string;
  address: string;
  units: number;
  occupied_units: number;
  monthly_rent_cents: number;
  monthly_rent_label: string;
  open_work: number;
  image_url: string | null;
}

export interface OwnerWork {
  id: string;
  title: string;
  property_id: string;
  property: string;
  status: string;
  waiting_on: string | null;
  priority: string;
  est_label: string;
  actual_label: string;
  created_at: string;
  resolved_at: string | null;
  approval: OwnerApproval | null;
}

export interface OwnerHome {
  owner_id: string;
  name: string;
  company: string;
  approval_limit_cents: number;
  approval_limit_label: string;
  entities: string[];
  properties: OwnerProperty[];
  pending: OwnerApproval[];
  open_work: OwnerWork[];
  last_month: string;
  month_to_date: {
    month: string;
    rent_collected_label: string;
    expenses_label: string;
    net_label: string;
  };
}

export interface EntityStatement {
  entity_id: string;
  entity_name: string;
  period_start: string;
  period_end: string;
  rent_collected_cents: number;
  rent_collected_label: string;
  expense_lines: { name: string; amount_cents: number; amount_label: string }[];
  expenses_cents: number;
  expenses_label: string;
  mgmt_fee_cents: number;
  mgmt_fee_label: string;
  net_cents: number;
  net_label: string;
}

export interface OwnerStatement {
  owner_id: string;
  owner_name: string;
  month: string;
  period_start: string;
  period_end: string;
  entities: EntityStatement[];
  rent_collected_label: string;
  expenses_label: string;
  mgmt_fee_label: string;
  net_label: string;
  net_cents: number;
  work: {
    ticket_id: string;
    title: string;
    property: string;
    resolved_on: string;
    cost_label: string;
    status: string;
  }[];
  approvals: {
    id: string;
    kind: string;
    title: string;
    amount_label: string;
    status: string;
    decided_at: string | null;
  }[];
}

export interface PublicApproval {
  id: string;
  company: string;
  owner_name: string;
  kind: string;
  status: string;
  amount_label: string;
  limit_label: string | null;
  title: string;
  description: string | null;
  property: string;
  note: string | null;
  requested_at: string;
  decided_at: string | null;
  decision_note: string | null;
  tasks: string[];
  photos: { id: string; url: string | null; kind: string; filename: string }[];
  updates: string[];
}

export interface TicketApprovals {
  owner_id: string | null;
  owner_name: string | null;
  limit_cents: number;
  limit_label: string;
  est_total_cents: number;
  est_total_label: string;
  needs_approval: boolean;
  approvals: OwnerApproval[];
}

const post = <T>(path: string, body: unknown, auth = true) =>
  request<T>(path, { method: "POST", auth, body });

export const owner = {
  home: () => request<OwnerHome>("/my/owner", { auth: true }),
  work: (all = false) =>
    request<OwnerWork[]>(`/my/owner/work${all ? "?all=true" : ""}`, {
      auth: true,
    }),
  approvals: () =>
    request<OwnerApproval[]>("/my/owner/approvals", { auth: true }),
  decide: (id: string, approve: boolean, note?: string) =>
    post<OwnerApproval>(`/my/owner/approvals/${id}`, { approve, note }),
  statement: (month?: string) =>
    request<OwnerStatement>(
      `/my/owner/statement${month ? `?month=${month}` : ""}`,
      { auth: true }
    ),
  // Staff, on a work order.
  forTicket: (ticketId: string) =>
    request<TicketApprovals>(`/tickets/${ticketId}/approvals`, { auth: true }),
  request: (ticketId: string, body: { amount_cents?: number; note?: string }) =>
    post<OwnerApproval>(`/tickets/${ticketId}/approvals`, body),
  staffDecide: (id: string, approve: boolean, note?: string) =>
    post<OwnerApproval>(`/approvals/${id}/decide`, { approve, note }),
  invite: (ownerId: string) =>
    post<{ owner_id: string; user_id: string; outcome: string }>(
      `/crm/owners/${ownerId}/invite`,
      {}
    ),
  // The link.
  publicView: (token: string) =>
    request<PublicApproval>(`/public/approve/${token}`),
  publicDecide: (token: string, approve: boolean, note?: string) =>
    post<PublicApproval>(`/public/approve/${token}`, { approve, note }, false),
};

/** An account whose only memberships here are owner-style ("landlord"). */
export function isOwnerOnly(
  user: Pick<User, "is_platform_staff" | "memberships"> | null | undefined
): boolean {
  if (!user || user.is_platform_staff) return false;
  const active = user.memberships.filter((m) => m.status !== "revoked");
  return (
    active.length > 0 && active.every((m) => m.profile_type === "landlord")
  );
}

/** "2026-10" → "October 2026". */
export function monthName(month: string): string {
  const [y, m] = month.split("-").map(Number);
  if (!y || !m) return month;
  return new Date(y, m - 1, 1).toLocaleDateString(undefined, {
    month: "long",
    year: "numeric",
  });
}

/** The month before `YYYY-MM`. */
export function previousMonth(month: string): string {
  const [y, m] = month.split("-").map(Number);
  const d = new Date(y, m - 2, 1);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

/** The answer in words. */
export function approvalWords(a: { kind: string; status: string }): string {
  if (a.kind === "approval") {
    switch (a.status) {
      case "pending":
        return "Waiting on you";
      case "approved":
        return "Approved";
      case "declined":
        return "Declined";
      case "overridden":
        return "Went ahead (emergency)";
      default:
        return a.status;
    }
  }
  switch (a.status) {
    case "pending":
      return "Sign-off needed";
    case "approved":
      return "Signed off";
    case "disputed":
      return "Disputed";
    default:
      return a.status;
  }
}
