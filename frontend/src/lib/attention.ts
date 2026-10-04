// Needs attention, everywhere: the portfolio rollup, the maintenance page's
// "To schedule" list, and the code-required items a property should have as
// routines.

import { request } from "@/lib/api";
import type { MaintenancePlan, MaintenanceTicket } from "@/lib/types";

export type AttentionKind =
  "profile" | "routine" | "unscheduled" | "vendor" | "approval";

export interface AttentionItem {
  kind: AttentionKind;
  key: string;
  title: string;
  detail: string;
  property_id: string | null;
  property_name: string | null;
  href: string;
  due_on: string | null;
  priority: "low" | "normal" | "high";
}

export interface AttentionResp {
  items: AttentionItem[];
  counts: Partial<Record<AttentionKind, number>>;
}

export interface DuePlan {
  plan: MaintenancePlan;
  property_name: string;
  /** Days until due; negative when overdue. */
  days: number;
  mandate: boolean;
}

export interface ToSchedule {
  plans: DuePlan[];
  tickets: MaintenanceTicket[];
  lead_days: number;
}

export interface MandateStatus {
  key: string;
  title: string;
  description: string;
  basis: string;
  cadence_days: number;
  category: string;
  priority: string;
  kit_key: string | null;
  /** Only matters if the property has the thing (a pool, a boiler). */
  conditional: boolean;
  plan_id: string | null;
  next_due_date: string | null;
  active: boolean | null;
}

export const KIND_WORDS: Record<AttentionKind, string> = {
  profile: "Property records",
  routine: "Routines to schedule",
  unscheduled: "Work orders with no date",
  vendor: "Vendor paperwork",
  approval: "Owner approvals",
};

export const attention = {
  portfolio: () => request<AttentionResp>("/attention", { auth: true }),
  toSchedule: () => request<ToSchedule>("/to-schedule", { auth: true }),
  runNow: (planId: string) =>
    request<MaintenanceTicket>(`/maintenance-plans/${planId}/run-now`, {
      auth: true,
      method: "POST",
    }),
  mandates: (propertyId: string) =>
    request<MandateStatus[]>(
      `/mandates?property_id=${encodeURIComponent(propertyId)}`,
      { auth: true }
    ),
  applyMandates: (propertyId: string, keys: string[] = []) =>
    request<{ created: number; items: MandateStatus[] }>(
      `/properties/${propertyId}/mandates`,
      { auth: true, method: "POST", body: { keys } }
    ),
};

/** "3 days overdue", "due today", "due in 12 days". */
export function dueWords(days: number): string {
  if (days < 0) return `${-days} day${days === -1 ? "" : "s"} overdue`;
  if (days === 0) return "due today";
  return `due in ${days} day${days === 1 ? "" : "s"}`;
}

/** "Every 6 months", "Yearly", "Every 2 years", "Every 45 days". */
export function cadenceWords(days: number): string {
  if (days === 365) return "Yearly";
  if (days === 30) return "Monthly";
  if (days === 182 || days === 183) return "Every 6 months";
  if (days === 90 || days === 91) return "Quarterly";
  if (days % 365 === 0) return `Every ${days / 365} years`;
  if (days > 365) return `Every ${Math.round((days / 365) * 10) / 10} years`;
  return `Every ${days} days`;
}

/** Items grouped by kind, in the order the page shows them. */
export function groupByKind(
  items: AttentionItem[]
): [AttentionKind, AttentionItem[]][] {
  const order: AttentionKind[] = [
    "approval",
    "unscheduled",
    "routine",
    "profile",
    "vendor",
  ];
  const m = new Map<AttentionKind, AttentionItem[]>();
  for (const i of items) m.set(i.kind, [...(m.get(i.kind) ?? []), i]);
  return order.filter((k) => m.has(k)).map((k) => [k, m.get(k)!]);
}
