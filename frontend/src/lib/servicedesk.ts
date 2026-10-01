// The service desk: job kits, tasks on a work order, costs against the
// estimate, photos and receipts, expenses, and vendors by trade.

import { ApiError, request } from "@/lib/api";
import type { MaintenancePlan, MaintenanceTicket } from "@/lib/types";

export const TRADES = [
  "general",
  "demo",
  "carpentry",
  "plumbing",
  "electrical",
  "hvac",
  "drywall",
  "paint",
  "tile",
  "flooring",
  "roofing",
  "appliance",
  "landscaping",
  "pest",
  "cleaning",
  "exterior",
] as const;

export type Trade = (typeof TRADES)[number];

export interface KitTask {
  title: string;
  trade: string;
  est_minutes: number | null;
  needs_contractor: boolean;
}

export interface KitPart {
  name: string;
  quantity: number;
  inventory_item_id: string | null;
  unit_cost_cents: number | null;
}

export interface Kit {
  id: string;
  name: string;
  area: string | null;
  category: string;
  priority: string;
  description: string | null;
  est_minutes: number | null;
  checklist: string[];
  parts: KitPart[];
  tasks: KitTask[];
  trades: string[];
  contractor_trades: string[];
  est_labor_cents: number;
  est_parts_cents: number;
  est_total_cents: number;
  est_total_label: string;
  active: boolean;
}

export interface Task {
  id: string;
  position: number;
  title: string;
  trade: string;
  est_minutes: number | null;
  est_cost_cents: number | null;
  est_cost_label: string | null;
  needs_contractor: boolean;
  assignee_entity_id: string | null;
  assignee_name: string | null;
  status: "todo" | "doing" | "done" | "skipped";
  done_at: string | null;
  dispatched_at: string | null;
}

export interface TradeNeed {
  trade: string;
  open_tasks: number;
  covered: boolean;
}

export interface Costs {
  ticket_id: string;
  est_labor_cents: number;
  est_parts_cents: number;
  est_total_cents: number;
  est_total_label: string;
  lines_cents: number;
  expenses_cents: number;
  approved_quotes_cents: number;
  actual_total_cents: number;
  actual_total_label: string;
  variance_cents: number;
  variance_label: string;
  receipts: number;
  tasks_total: number;
  tasks_done: number;
  trades_needed: TradeNeed[];
}

export interface TicketFile {
  id: string;
  filename: string;
  mime_type: string;
  kind: "photo" | "receipt" | "document";
  size_bytes: number;
  url: string | null;
  created_at: string;
}

export interface TicketExpense {
  id: string;
  incurred_on: string;
  category: string;
  vendor: string | null;
  description: string;
  amount_cents: number;
  amount_label: string;
  billable_to_owner: boolean;
  reimbursable: boolean;
  receipt_document_ids: string[];
  created_at: string;
}

export interface VendorOption {
  id: string;
  name: string;
  email: string | null;
  phone: string | null;
  trades: string[];
  matches: boolean;
  coi_current: boolean;
  linked: boolean;
}

export interface Generated {
  ticket: MaintenanceTicket;
}

/** "plumbing" → "Plumbing"; "hvac" → "HVAC". */
export function tradeLabel(t: string): string {
  if (t === "hvac") return "HVAC";
  return t.charAt(0).toUpperCase() + t.slice(1);
}

/** 90 → "1h 30m"; 45 → "45m". */
export function minutesLabel(m: number | null | undefined): string {
  if (!m) return "";
  const h = Math.floor(m / 60);
  const r = m % 60;
  return h ? (r ? `${h}h ${r}m` : `${h}h`) : `${r}m`;
}

/** Whole dollars from cents. */
export function dollars(cents: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 0,
  }).format(Math.round(cents / 100));
}

/** Exact money from cents: whole dollars stay whole ("$149"), anything
 * else keeps its cents ("$214.55"). For money actually spent. */
export function money(cents: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: cents % 100 === 0 ? 0 : 2,
    maximumFractionDigits: 2,
  }).format(cents / 100);
}

/** "$214.55" → 21455; blank or junk → null. */
export function parseCents(raw: string): number | null {
  const v = Number(raw.replace(/[$,\s]/g, ""));
  if (!raw.trim() || !Number.isFinite(v) || v < 0) return null;
  return Math.round(v * 100);
}

const post = <T>(path: string, body: unknown) =>
  request<T>(path, { method: "POST", auth: true, body });
const patch = <T>(path: string, body: unknown) =>
  request<T>(path, { method: "PATCH", auth: true, body });

export const desk = {
  kits: () => request<Kit[]>("/issue-templates", { auth: true }),
  generate: (
    kitId: string,
    body: {
      property_id: string;
      unit_id?: string;
      note?: string;
      priority?: string;
    }
  ) => post<Generated>(`/issue-templates/${kitId}/generate`, body),
  tasks: (ticketId: string) =>
    request<Task[]>(`/tickets/${ticketId}/tasks`, { auth: true }),
  addTask: (
    ticketId: string,
    body: {
      title: string;
      trade?: string;
      est_minutes?: number;
      needs_contractor?: boolean;
    }
  ) => post<Task[]>(`/tickets/${ticketId}/tasks`, body),
  updateTask: (
    ticketId: string,
    taskId: string,
    body: Partial<{
      title: string;
      trade: string;
      est_minutes: number;
      needs_contractor: boolean;
      status: Task["status"];
      assignee_entity_id: string;
      position: number;
    }>
  ) => patch<Task[]>(`/tickets/${ticketId}/tasks/${taskId}`, body),
  removeTask: (ticketId: string, taskId: string) =>
    request<Task[]>(`/tickets/${ticketId}/tasks/${taskId}`, {
      method: "DELETE",
      auth: true,
    }),
  dispatchTask: (
    ticketId: string,
    taskId: string,
    body: { entity_id: string; note?: string; coi_override_reason?: string }
  ) => post<Task[]>(`/tickets/${ticketId}/tasks/${taskId}/dispatch`, body),
  applyKit: (ticketId: string, kitId: string) =>
    post<Task[]>(`/tickets/${ticketId}/kits`, { issue_template_id: kitId }),
  costs: (ticketId: string) =>
    request<Costs>(`/tickets/${ticketId}/costs`, { auth: true }),
  files: (ticketId: string) =>
    request<TicketFile[]>(`/tickets/${ticketId}/files`, { auth: true }),
  expenses: (ticketId: string) =>
    request<TicketExpense[]>(`/tickets/${ticketId}/expenses`, { auth: true }),
  addExpense: (
    ticketId: string,
    body: {
      description: string;
      amount_cents: number;
      vendor?: string;
      category?: string;
      incurred_on?: string;
      receipt_document_ids?: string[];
      billable_to_owner?: boolean;
      reimbursable?: boolean;
    }
  ) => post<TicketExpense>(`/tickets/${ticketId}/expenses`, body),
  vendors: (ticketId: string, trade?: string) =>
    request<VendorOption[]>(
      `/tickets/${ticketId}/vendors${trade ? `?trade=${encodeURIComponent(trade)}` : ""}`,
      { auth: true }
    ),
  note: (
    ticketId: string,
    body: {
      body: string;
      visibility: "public" | "internal";
      document_ids?: string[];
    }
  ) => post<unknown>(`/tickets/${ticketId}/comments`, body),
  /** Register a file on the work order, then PUT the bytes to the signed URL. */
  upload: async (
    ticketId: string,
    file: File,
    kind: "photo" | "receipt" | "document"
  ): Promise<TicketFile> => {
    const reg = await post<{ file: TicketFile; upload_url: string }>(
      `/tickets/${ticketId}/uploads`,
      {
        filename: file.name || `${kind}.jpg`,
        mime_type: file.type || "application/octet-stream",
        size_bytes: file.size,
        kind,
      }
    );
    const res = await fetch(reg.upload_url, {
      method: "PUT",
      body: file,
      headers: { "Content-Type": file.type || "application/octet-stream" },
    });
    if (!res.ok)
      throw new ApiError(res.status, "upload_failed", "upload failed");
    return reg.file;
  },
  plans: () => request<MaintenancePlan[]>("/maintenance-plans", { auth: true }),
  createPlan: (body: {
    property_id: string;
    title: string;
    cadence_days: number;
    next_due_date: string;
    category?: string;
    priority?: string;
    description?: string;
    issue_template_id?: string;
  }) => post<MaintenancePlan>("/maintenance-plans", body),
  updatePlan: (
    id: string,
    body: Partial<{
      title: string;
      cadence_days: number;
      next_due_date: string;
      active: boolean;
      issue_template_id: string;
    }>
  ) => patch<MaintenancePlan>(`/maintenance-plans/${id}`, body),
};
