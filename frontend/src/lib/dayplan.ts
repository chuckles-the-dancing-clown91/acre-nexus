// Plan the day: the shopping list by day and store, a proposed route for a
// technician, and accepting it.

import { request } from "@/lib/api";
import type { TicketPart } from "@/lib/types";

export interface ShopItem extends TicketPart {
  ticket_title: string;
  property_id: string;
  property_name: string;
  day: string | null;
}

export interface StoreGroup {
  store: string;
  items: ShopItem[];
  est_cents: number;
}

export interface LowStock {
  inventory_item_id: string;
  name: string;
  on_hand: number;
  after: number;
  reorder_level: number;
}

export interface DayShopping {
  day: string;
  stores: StoreGroup[];
  from_stock: ShopItem[];
}

export interface Shopping {
  from: string;
  to: string;
  days: DayShopping[];
  undated: StoreGroup[];
  low_stock: LowStock[];
  total_cents: number;
}

export interface Stop {
  kind: "store" | "job";
  ticket_id: string | null;
  title: string;
  property_id: string | null;
  property_name: string;
  address: string;
  priority: string;
  status: string;
  assignee_user_id: string | null;
  fixed: boolean;
  minutes: number;
  drive_minutes: number;
  start: string;
  end: string;
  when_words: string;
  tasks_total: number;
  tasks_done: number;
  to_buy: number;
  from_stock: number;
  with_name: string | null;
  with_phone: string | null;
  access_notes: string | null;
  note: string | null;
}

export interface Route {
  date: string;
  assignee_user_id: string | null;
  assignee_name: string | null;
  day_start: string;
  day_end: string;
  stops: Stop[];
  unplaced: Stop[];
  total_minutes: number;
  drive_minutes: number;
  stores: StoreGroup[];
  from_stock: ShopItem[];
  low_stock: LowStock[];
}

export interface Accepted {
  date: string;
  assignee_name: string;
  booked: number;
  kept: number;
  to_order: StoreGroup[];
  from_stock: ShopItem[];
  low_stock: LowStock[];
}

export const dayplan = {
  shopping: (from?: string, to?: string) => {
    const q = new URLSearchParams();
    if (from) q.set("from", from);
    if (to) q.set("to", to);
    const s = q.toString();
    return request<Shopping>(`/shopping${s ? `?${s}` : ""}`, { auth: true });
  },
  propose: (body: {
    date?: string;
    assignee_user_id?: string;
    start?: string;
  }) => request<Route>("/routes/propose", { auth: true, method: "POST", body }),
  accept: (body: {
    date: string;
    assignee_user_id: string;
    stops: { ticket_id: string; start: string; end: string }[];
  }) =>
    request<Accepted>("/routes/accept", { auth: true, method: "POST", body }),
};

/** "1h 30m", "45m", "2h". */
export function hours(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m}m`;
  if (m === 0) return `${h}h`;
  return `${h}h ${m}m`;
}

/** $ with cents dropped when zero: "$42", "$42.50". */
export function dollars(cents: number): string {
  const d = cents / 100;
  return Number.isInteger(d)
    ? `$${d.toLocaleString()}`
    : `$${d.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

/** A search page at the store for a part with no link of its own. */
export function storeSearch(store: string, name: string): string | null {
  const q = encodeURIComponent(name);
  switch (store) {
    case "Home Depot":
      return `https://www.homedepot.com/s/${q}`;
    case "Lowe's":
      return `https://www.lowes.com/search?searchTerm=${q}`;
    case "Amazon":
      return `https://www.amazon.com/s?k=${q}`;
    case "Walmart":
      return `https://www.walmart.com/search?q=${q}`;
    case "Menards":
      return `https://www.menards.com/main/search.html?search=${q}`;
    case "Ace Hardware":
      return `https://www.acehardware.com/search?query=${q}`;
    case "Grainger":
      return `https://www.grainger.com/search?searchQuery=${q}`;
    case "SupplyHouse":
      return `https://www.supplyhouse.com/sh/control/search?searchTerm=${q}`;
    case "Ferguson":
      return `https://www.ferguson.com/search/${q}`;
    default:
      return null;
  }
}

/** The stops as the accept call wants them: jobs only. */
export function stopsToAccept(
  stops: Stop[]
): { ticket_id: string; start: string; end: string }[] {
  return stops
    .filter((s) => s.kind === "job" && s.ticket_id)
    .map((s) => ({ ticket_id: s.ticket_id!, start: s.start, end: s.end }));
}

/** Days from `from` through `to` inclusive, as YYYY-MM-DD. */
export function dayRange(from: string, to: string): string[] {
  const out: string[] = [];
  const d = new Date(`${from}T00:00:00`);
  const end = new Date(`${to}T00:00:00`);
  while (d <= end && out.length < 62) {
    out.push(
      `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`
    );
    d.setDate(d.getDate() + 1);
  }
  return out;
}
