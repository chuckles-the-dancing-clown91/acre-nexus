// Operations analytics, the leasing funnel, and the portfolio map.

import { request } from "@/lib/api";

export interface TurnStats {
  count: number;
  avg_days: number | null;
  avg_cost_cents: number | null;
  total_cost_cents: number;
}

export interface TicketStats {
  opened: number;
  resolved: number;
  past_sla: number;
  avg_rating: number | null;
  avg_hours_to_resolve: number | null;
  spend_cents: number;
}

export interface MonthRow {
  month: string;
  turns: TurnStats;
  tickets: TicketStats;
}

export interface PropertyRow {
  property_id: string;
  name: string;
  units: number;
  occupied: number;
  turns: TurnStats;
  tickets: TicketStats;
}

export interface RepeatIssue {
  property_id: string;
  property_name: string;
  category: string;
  count: number;
  spend_cents: number;
  last_opened: string;
}

export interface ApplianceSpend {
  asset_id: string;
  name: string;
  kind: string;
  property_id: string;
  property_name: string;
  repairs: number;
  spend_cents: number;
  price_cents: number | null;
  share_pct: number | null;
  replace: boolean;
}

export interface Operations {
  from: string;
  to: string;
  totals: MonthRow;
  months: MonthRow[];
  properties: PropertyRow[];
  categories: { category: string; count: number; spend_cents: number }[];
  repeats: RepeatIssue[];
  appliances: ApplianceSpend[];
  replace_share_pct: number;
  turns_open: number;
  tickets_open: number;
}

export interface ListingDays {
  listing_id: string;
  title: string;
  status: string;
  rent_cents: number;
  listed_on: string;
  days_on_market: number;
  leased: boolean;
  tours: number;
  applications: number;
}

export interface Leasing {
  from: string;
  tours: number;
  applications: number;
  approved: number;
  leases: number;
  tour_to_application_pct: number | null;
  application_to_lease_pct: number | null;
  avg_days_on_market: number | null;
  listings: ListingDays[];
}

export interface MapPin {
  property_id: string;
  name: string;
  address: string;
  city: string;
  state: string;
  lat: number | null;
  lng: number | null;
  units: number;
  occupied: number;
  occupancy_pct: number | null;
  open_tickets: number;
  urgent_tickets: number;
  open_turns: number;
  site_map_id: string | null;
  image_url: string | null;
  monthly_rent_cents: number;
}

export const analytics = {
  operations: (months: number, propertyId?: string) => {
    const q = new URLSearchParams({ months: String(months) });
    if (propertyId) q.set("property_id", propertyId);
    return request<Operations>(`/analytics/operations?${q}`, { auth: true });
  },
  leasing: (months: number) =>
    request<Leasing>(`/analytics/leasing?months=${months}`, { auth: true }),
  map: () =>
    request<{ pins: MapPin[]; unplaced: number }>("/portfolio/map", {
      auth: true,
    }),
};

/** How a pin is coloured: by occupancy, or by open work. */
export type PinMode = "occupancy" | "work";

export type PinTone = "good" | "warn" | "bad" | "neutral";

export function pinTone(p: MapPin, mode: PinMode): PinTone {
  if (mode === "work") {
    if (p.urgent_tickets > 0) return "bad";
    if (p.open_tickets + p.open_turns > 0) return "warn";
    return "good";
  }
  if (p.occupancy_pct === null) return "neutral";
  if (p.occupancy_pct >= 95) return "good";
  if (p.occupancy_pct >= 85) return "warn";
  return "bad";
}

export function hours(h: number | null): string {
  if (h === null) return "—";
  if (h < 48) return `${Math.round(h)} h`;
  return `${(h / 24).toFixed(1)} d`;
}
