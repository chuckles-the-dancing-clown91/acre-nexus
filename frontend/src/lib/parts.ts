// The parts loop, stock, appliances and the Alpha link (Vantedge phase 2C):
// typed helpers over the maintenance API. Money is cents; dates are ISO.

import { request, type DocumentEntry } from "@/lib/api";
import type {
  Asset,
  InventoryItem,
  MaintenancePlan,
  MaintenanceTicket,
  TicketComment,
  TicketLine,
  TicketPart,
} from "@/lib/types";

export interface PartInput {
  inventory_item_id?: string;
  name?: string;
  quantity?: number;
  note?: string;
}

export interface Finding extends TicketComment {
  parts: TicketPart[];
}

export interface PartsList {
  ticket_id: string;
  title: string;
  property: string;
  from_stock: TicketPart[];
  to_buy: TicketPart[];
  maybe: TicketPart[];
  coming: TicketPart[];
}

export interface CloseoutTicket {
  ticket_id: string;
  title: string;
  property_id: string;
  property: string;
  property_address: string;
  status: string;
  priority: string;
  due_date: string | null;
  assignee: string | null;
  parts: TicketPart[];
}

export interface Closeout {
  date: string;
  tickets: CloseoutTicket[];
  to_decide: number;
  on_order: TicketPart[];
}

export interface DecideInput {
  action: "order" | "pick_up" | "from_stock" | "skip";
  vendor?: string;
  tracking?: string;
  ship_to?: "property" | "office" | "other";
  ship_to_note?: string;
  unit_cost_cents?: number;
  billable_to_owner?: boolean;
  need_by?: string;
}

export interface AssetPart {
  id: string;
  inventory_item_id: string;
  name: string;
  sku: string | null;
  quantity: number;
  role: string | null;
  note: string | null;
  in_stock: number;
  unit_cost_cents: number | null;
}

export interface AssetHistory extends Asset {
  property: string;
  tickets: MaintenanceTicket[];
  plans: MaintenancePlan[];
  documents: DocumentEntry[];
  parts: AssetPart[];
  spend_cents: number;
  spend_label: string;
  last_serviced: string | null;
}

export interface Movement {
  id: string;
  inventory_item_id: string;
  item_name: string;
  kind: "receive" | "use" | "count" | "restock";
  quantity: number;
  unit_cost_cents: number;
  value_cents: number;
  ticket_id: string | null;
  ticket_title: string | null;
  expense_id: string | null;
  note: string | null;
  recorded_by: string | null;
  created_at: string;
}

export interface ReceiveLine {
  inventory_item_id?: string;
  new_item?: {
    name: string;
    barcode?: string;
    sku?: string;
    category?: string;
    unit?: string;
    reorder_level?: number;
    storage_location?: string;
  };
  quantity: number;
  unit_cost_cents: number;
}

export interface ReceiveResult {
  expense_id: string;
  items: InventoryItem[];
  total_cents: number;
}

export interface ReorderGroup {
  vendor: string;
  items: InventoryItem[];
}

export interface PartnerLink {
  counterparty_id: string;
  linked: boolean;
  kind: string | null;
  base_url: string | null;
  linked_at: string | null;
  status: string | null;
  error: string | null;
  callback_url: string;
  callback_secret_set: boolean;
}

export interface LinkResult extends PartnerLink {
  callback_secret: string | null;
}

export interface LinkedVendor {
  id: string;
  name: string;
  kind: string;
  partner_kind: string;
  status: string | null;
}

const post = <T>(path: string, body: unknown = {}) =>
  request<T>(path, { method: "POST", auth: true, body });

export const parts = {
  list: (ticketId: string) =>
    request<TicketPart[]>(`/tickets/${ticketId}/parts`, { auth: true }),
  add: (
    ticketId: string,
    body: PartInput & { status?: "potential" | "needed" }
  ) => post<TicketPart>(`/tickets/${ticketId}/parts`, body),
  update: (
    id: string,
    body: {
      name?: string;
      quantity?: number;
      status?: string;
      need_by?: string;
      note?: string;
    }
  ) =>
    request<TicketPart>(`/parts/${id}`, { method: "PATCH", auth: true, body }),
  remove: (id: string) =>
    request<{ ok: boolean }>(`/parts/${id}`, { method: "DELETE", auth: true }),
  findings: (ticketId: string) =>
    request<Finding[]>(`/tickets/${ticketId}/findings`, { auth: true }),
  addFinding: (
    ticketId: string,
    body: {
      body: string;
      visibility?: "public" | "internal";
      parts: PartInput[];
    }
  ) => post<Finding>(`/tickets/${ticketId}/findings`, body),
  generate: (ticketId: string) =>
    post<PartsList>(`/tickets/${ticketId}/parts/generate`),
  pdfPath: (ticketId: string) => `/tickets/${ticketId}/parts/list.pdf`,
  closeout: (date?: string) =>
    request<Closeout>(`/closeout${date ? `?date=${date}` : ""}`, {
      auth: true,
    }),
  decide: (id: string, body: DecideInput) =>
    post<TicketPart>(`/parts/${id}/decide`, body),
  receive: (id: string) => post<TicketPart>(`/parts/${id}/receive`),
  use: (id: string) => post<TicketLine>(`/parts/${id}/use`),
};

export const appliances = {
  history: (id: string) =>
    request<AssetHistory>(`/assets/${id}/history`, { auth: true }),
  parts: (id: string) =>
    request<AssetPart[]>(`/assets/${id}/parts`, { auth: true }),
  putPart: (
    id: string,
    body: {
      inventory_item_id: string;
      quantity?: number;
      role?: string;
      note?: string;
    }
  ) =>
    request<AssetPart[]>(`/assets/${id}/parts`, {
      method: "PUT",
      auth: true,
      body,
    }),
  removePart: (id: string, partId: string) =>
    request<AssetPart[]>(`/assets/${id}/parts/${partId}`, {
      method: "DELETE",
      auth: true,
    }),
  workOrder: (
    id: string,
    body: {
      kind: "repair" | "replace" | "service";
      description?: string;
      priority?: string;
      due_date?: string;
    }
  ) =>
    post<{ ticket_id: string; title: string; potential_parts: number }>(
      `/assets/${id}/work-order`,
      body
    ),
};

export const stock = {
  lookup: (code: string) =>
    request<InventoryItem>(
      `/inventory/lookup?code=${encodeURIComponent(code)}`,
      { auth: true }
    ),
  receive: (body: {
    lines: ReceiveLine[];
    vendor?: string;
    incurred_on?: string;
    total_cents?: number;
    note?: string;
  }) => post<ReceiveResult>("/inventory/receive", body),
  count: (id: string, counted: number, note?: string) =>
    post<InventoryItem>(`/inventory/${id}/count`, { counted, note }),
  movements: (
    params: { item_id?: string; ticket_id?: string; kind?: string } = {}
  ) => {
    const qs = new URLSearchParams();
    for (const [k, v] of Object.entries(params)) if (v) qs.set(k, v);
    const suffix = qs.toString() ? `?${qs}` : "";
    return request<Movement[]>(`/inventory/movements${suffix}`, { auth: true });
  },
  reorder: () => request<ReorderGroup[]>("/inventory/reorder", { auth: true }),
};

export const partner = {
  link: (counterpartyId: string) =>
    request<PartnerLink>(`/entities/${counterpartyId}/partner`, { auth: true }),
  connect: (
    counterpartyId: string,
    body: { base_url: string; api_key: string }
  ) => post<LinkResult>(`/entities/${counterpartyId}/partner/link`, body),
  disconnect: (counterpartyId: string) =>
    request<PartnerLink>(`/entities/${counterpartyId}/partner/link`, {
      method: "DELETE",
      auth: true,
    }),
  rotateSecret: (counterpartyId: string) =>
    post<LinkResult>(`/entities/${counterpartyId}/partner/secret`),
  vendors: () => request<LinkedVendor[]>("/partner/vendors", { auth: true }),
  dispatch: (
    ticketId: string,
    body: {
      counterparty_id: string;
      requested_for?: string;
      service_key?: string;
      note?: string;
    }
  ) => post<MaintenanceTicket>(`/tickets/${ticketId}/dispatch`, body),
};

/** Human label for a part's place in the loop. */
export const PART_LABELS: Record<string, string> = {
  potential: "might need",
  needed: "needed",
  from_stock: "from stock",
  to_order: "to buy",
  ordered: "ordered",
  pick_up: "pick up",
  received: "received",
  used: "used",
  skipped: "skipped",
};

export function partTone(
  status: string
): "good" | "warn" | "bad" | "info" | "neutral" {
  switch (status) {
    case "used":
    case "received":
    case "from_stock":
      return "good";
    case "to_order":
    case "needed":
      return "warn";
    case "ordered":
    case "pick_up":
      return "info";
    case "skipped":
      return "neutral";
    default:
      return "neutral";
  }
}
