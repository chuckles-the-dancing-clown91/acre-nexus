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
  /** Product page (Home Depot, Lowe's, Amazon, ...). */
  url?: string | null;
}

export interface Kit {
  id: string;
  /** Stable key for a catalog kit (`replace-dishwasher`). */
  kit_key?: string | null;
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
  /** The vendor's name. */
  assignee_name: string | null;
  /** A person on the team doing it. */
  assignee_user_id: string | null;
  assignee_user_name: string | null;
  status: "todo" | "doing" | "done" | "skipped";
  done_at: string | null;
  dispatched_at: string | null;
  /** How it reached the vendor: their own board, or by email. */
  dispatch_via: "partner" | "email" | null;
  dispatch_note: string | null;
  /** The vendor's answer from their link. */
  vendor_response: "accepted" | "declined" | "done" | null;
  vendor_responded_at: string | null;
  vendor_note: string | null;
}

/** A teammate who can be given work, with what they already have. */
export interface Tech {
  user_id: string;
  name: string;
  email: string;
  role: string;
  title: string | null;
  open_tickets: number;
  open_tasks: number;
  /** Assigned to the property asked about. */
  on_property: boolean;
}

/** One of the caller's own tasks, with the work order it belongs to. */
export interface QueueTask {
  task_id: string;
  ticket_id: string;
  title: string;
  trade: string;
  est_minutes: number | null;
  status: "todo" | "doing";
  ticket_title: string;
  property_id: string;
  property_name: string;
  location: string | null;
  priority: string;
  due_date: string | null;
  ticket_status: string;
  waiting_on: string | null;
}

/** "Maintenance" → "maintenance"; "property_manager" → "Property manager". */
export function roleLabel(r: string): string {
  const s = r.replace(/_/g, " ");
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** "3 open" or "free": what a teammate already has. */
export function loadLabel(
  t: Pick<Tech, "open_tickets" | "open_tasks">
): string {
  const parts = [
    t.open_tickets &&
      `${t.open_tickets} work ${t.open_tickets === 1 ? "order" : "orders"}`,
    t.open_tasks && `${t.open_tasks} ${t.open_tasks === 1 ? "task" : "tasks"}`,
  ].filter(Boolean);
  return parts.length ? parts.join(", ") : "free";
}

/** The queue views, and which work orders each shows. */
export type QueueView =
  "mine" | "open" | "unassigned" | "urgent" | "waiting" | "done";

const OPEN_STATUSES = ["open", "triage", "scheduled", "in_progress", "on_hold"];

export function inQueueView(
  t: {
    status: string;
    priority: string;
    waiting_on: string | null;
    assignee_user_id: string | null;
    assignee_entity_id: string | null;
  },
  view: QueueView,
  me: string | null
): boolean {
  const open = OPEN_STATUSES.includes(t.status);
  switch (view) {
    case "mine":
      return open && !!me && t.assignee_user_id === me;
    case "open":
      return open;
    case "unassigned":
      return open && !t.assignee_user_id && !t.assignee_entity_id;
    case "urgent":
      return open && (t.priority === "urgent" || t.priority === "high");
    case "waiting":
      return t.status === "on_hold" || !!t.waiting_on;
    case "done":
      return !open;
  }
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
  kind: "photo" | "video" | "receipt" | "document";
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
  /** When we last invited them to sign up for Alpha. */
  alpha_invited_at: string | null;
}

/** A one-press update on a work order: posts the note and moves the status. */
export interface TicketAction {
  key: string;
  label: string;
  says: string;
  visibility: "public" | "internal";
  status: string | null;
  waiting_on: string | null;
  needs_note: boolean;
}

/** Where to buy a part: its own link if it has one, else store searches. */
export function partLinks(p: {
  name: string;
  url?: string | null;
  store?: string | null;
}): { label: string; href: string }[] {
  if (p.url)
    return [{ label: p.store ? `Buy at ${p.store}` : "Buy", href: p.url }];
  const q = encodeURIComponent(p.name);
  return [
    { label: "Home Depot", href: `https://www.homedepot.com/s/${q}` },
    { label: "Lowe's", href: `https://www.lowes.com/search?searchTerm=${q}` },
    { label: "Amazon", href: `https://www.amazon.com/s?k=${q}` },
  ];
}

/** True for a link the server will take: http(s), no spaces. */
export function looksLikeUrl(raw: string): boolean {
  const v = raw.trim();
  if (!v) return true;
  if (/\s/.test(v)) return false;
  try {
    const u = new URL(v);
    return u.protocol === "https:" || u.protocol === "http:";
  } catch {
    return false;
  }
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

export const KIT_CATEGORIES = [
  "plumbing",
  "electrical",
  "hvac",
  "appliance",
  "structural",
  "general",
] as const;

export const PRIORITIES = ["low", "normal", "high", "urgent"] as const;

/** A kit as it's being edited: strings for the number fields so a blank stays
 * blank while typing. */
export interface KitDraft {
  name: string;
  area: string;
  category: string;
  priority: string;
  description: string;
  tasks: {
    title: string;
    trade: string;
    minutes: string;
    needs_contractor: boolean;
  }[];
  parts: {
    name: string;
    quantity: string;
    cost: string;
    url: string;
    inventory_item_id: string | null;
  }[];
}

/** What the server takes to save a kit. */
export interface KitReq {
  name: string;
  area: string | null;
  category: string;
  priority: string;
  description: string | null;
  est_minutes: number | null;
  checklist: string[];
  tasks: KitTask[];
  parts: KitPart[];
}

export function emptyDraft(): KitDraft {
  return {
    name: "",
    area: "",
    category: "general",
    priority: "normal",
    description: "",
    tasks: [],
    parts: [],
  };
}

/** Start a draft from a saved kit. An older catalog entry with only a
 * checklist gets its checklist as tasks, so saving moves it to tasks. */
export function draftFrom(kit: Kit): KitDraft {
  const tasks = kit.tasks.length
    ? kit.tasks.map((t) => ({
        title: t.title,
        trade: t.trade,
        minutes: t.est_minutes ? String(t.est_minutes) : "",
        needs_contractor: t.needs_contractor,
      }))
    : kit.checklist.map((c) => ({
        title: c,
        trade: "general",
        minutes: "",
        needs_contractor: false,
      }));
  return {
    name: kit.name,
    area: kit.area ?? "",
    category: kit.category,
    priority: kit.priority,
    description: kit.description ?? "",
    tasks,
    parts: kit.parts.map((p) => ({
      name: p.name,
      quantity: String(p.quantity),
      cost:
        p.unit_cost_cents != null ? (p.unit_cost_cents / 100).toFixed(2) : "",
      url: p.url ?? "",
      inventory_item_id: p.inventory_item_id,
    })),
  };
}

function wholeMinutes(raw: string): number | null {
  const n = Math.round(Number(raw));
  return raw.trim() && Number.isFinite(n) && n > 0 ? n : null;
}

/** The draft as the server takes it: blank lines dropped, numbers parsed. */
export function draftToReq(d: KitDraft): KitReq {
  const tasks: KitTask[] = d.tasks
    .filter((t) => t.title.trim())
    .map((t) => ({
      title: t.title.trim(),
      trade: t.trade,
      est_minutes: wholeMinutes(t.minutes),
      needs_contractor: t.needs_contractor,
    }));
  const parts: KitPart[] = d.parts
    .filter((p) => p.name.trim())
    .map((p) => ({
      name: p.name.trim(),
      quantity: Math.max(1, Math.round(Number(p.quantity)) || 1),
      inventory_item_id: p.inventory_item_id,
      unit_cost_cents: parseCents(p.cost),
      url: p.url.trim() || null,
    }));
  const minutes = tasks.reduce((s, t) => s + (t.est_minutes ?? 0), 0);
  return {
    name: d.name.trim(),
    area: d.area.trim() || null,
    category: d.category,
    priority: d.priority,
    description: d.description.trim() || null,
    est_minutes: minutes || null,
    checklist: [],
    tasks,
    parts,
  };
}

/** Running totals while editing: time in-house and by contractors, parts. */
export function draftTotals(d: KitDraft): {
  minutes: number;
  contractorMinutes: number;
  partsCents: number;
} {
  const r = draftToReq(d);
  let minutes = 0;
  let contractorMinutes = 0;
  for (const t of r.tasks) {
    if (t.needs_contractor) contractorMinutes += t.est_minutes ?? 0;
    else minutes += t.est_minutes ?? 0;
  }
  const partsCents = r.parts.reduce(
    (s, p) => s + (p.unit_cost_cents ?? 0) * p.quantity,
    0
  );
  return { minutes, contractorMinutes, partsCents };
}

export const desk = {
  kits: () => request<Kit[]>("/issue-templates", { auth: true }),
  createKit: (body: KitReq) => post<Kit>("/issue-templates", body),
  updateKit: (id: string, body: KitReq) =>
    request<Kit>(`/issue-templates/${id}`, { method: "PUT", auth: true, body }),
  retireKit: (id: string) =>
    request<{ ok: boolean }>(`/issue-templates/${id}`, {
      method: "DELETE",
      auth: true,
    }),
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
      /** A teammate's user id, or "" to clear. */
      assignee_user_id: string;
      position: number;
    }>
  ) => patch<Task[]>(`/tickets/${ticketId}/tasks/${taskId}`, body),
  techs: (propertyId?: string) =>
    request<Tech[]>(
      `/ticket-techs${propertyId ? `?property_id=${propertyId}` : ""}`,
      { auth: true }
    ),
  queue: () => request<{ tasks: QueueTask[] }>("/ticket-queue", { auth: true }),
  /** Several tasks to one vendor, as one job. */
  dispatchTasks: (
    ticketId: string,
    body: {
      task_ids: string[];
      entity_id: string;
      note?: string;
      coi_override_reason?: string;
    }
  ) => post<Task[]>(`/tickets/${ticketId}/dispatch-tasks`, body),
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
  /** Invite a vendor to sign up for Alpha, so work lands on their own board. */
  alphaInvite: (entityId: string) =>
    post<{ alpha_invited_at: string; join_url: string }>(
      `/entities/${entityId}/alpha-invite`,
      {}
    ),
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
    kind: "photo" | "video" | "receipt" | "document"
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
  actions: () => request<TicketAction[]>("/ticket-actions", { auth: true }),
  press: (
    ticketId: string,
    body: { action: string; note?: string; follow_up_date?: string }
  ) => post<MaintenanceTicket>(`/tickets/${ticketId}/actions`, body),
  updatePart: (partId: string, body: { url?: string; vendor?: string }) =>
    patch<unknown>(`/parts/${partId}`, body),
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
