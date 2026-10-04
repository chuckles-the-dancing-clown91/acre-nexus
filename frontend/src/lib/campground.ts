// Campground reservations: rules, seasons, sites, stays, the front desk, and
// the guest booking pages.

import { DEFAULT_TENANT, request } from "@/lib/api";

export interface Addon {
  key: string;
  label: string;
  price_cents: number;
  per: "stay" | "night";
}

export interface CampConfig {
  booking_open: boolean;
  deposit_pct: number;
  check_in_time: string;
  check_out_time: string;
  max_nights: number;
  addons: Addon[];
  policies: string | null;
}

export interface Season {
  id: string;
  name: string;
  start_md: string;
  end_md: string;
  adjust_pct: number;
  min_nights: number;
}

export interface Site {
  id: string;
  name: string;
  site_type: string | null;
  max_length_ft: number | null;
  max_guests: number | null;
  power_amps: number | null;
  water: boolean;
  sewer: boolean;
  pull_through: boolean;
  pets: boolean;
  rate_cents_night: number | null;
  rate_cents_week: number | null;
  rate_cents_month: number | null;
  closed: boolean;
}

export interface AddonLine {
  key: string;
  label: string;
  qty: number;
  cents: number;
}

export type StayStatus =
  "held" | "confirmed" | "checked_in" | "checked_out" | "cancelled";

export interface Stay {
  id: string;
  map_id: string;
  site_id: string;
  site_name: string;
  guest_name: string;
  email: string | null;
  phone: string | null;
  check_in: string;
  check_out: string;
  nights: number;
  guests: number;
  vehicle: string | null;
  rig_length_ft: number | null;
  addons: AddonLine[];
  total_cents: number;
  deposit_cents: number;
  paid_cents: number;
  balance_cents: number;
  status: StayStatus;
  source: string;
  note: string | null;
  checked_in_at: string | null;
  checked_out_at: string | null;
  cleaned_at: string | null;
}

export interface Quote {
  nights: number;
  months: number;
  weeks: number;
  single_nights: number;
  base_cents: number;
  season_pct: number;
  seasons: string[];
  stay_cents: number;
  addons: AddonLine[];
  total_cents: number;
  deposit_cents: number;
  min_nights: number;
}

export interface CampSummary {
  map_id: string;
  name: string;
  property_id: string;
  property_name: string;
  sites: number;
  published: boolean;
  booking_open: boolean;
  in_house: number;
  arriving_today: number;
}

export interface CampDetail {
  map_id: string;
  name: string;
  property_id: string;
  published: boolean;
  config: CampConfig;
  seasons: Season[];
  sites: Site[];
}

export interface Board {
  date: string;
  arriving: Stay[];
  departing: Stay[];
  in_house: Stay[];
  to_clean: Stay[];
  requests: Stay[];
  sites: number;
  occupied: number;
}

export interface QuoteInput {
  site_id: string;
  check_in: string;
  check_out: string;
  addons: [string, number][];
}

export interface StayInput extends QuoteInput {
  guest_name: string;
  email?: string;
  phone?: string;
  guests?: number;
  vehicle?: string;
  rig_length_ft?: number;
  note?: string;
  website?: string;
}

export type StayActionName =
  "confirm" | "check_in" | "check_out" | "cancel" | "cleaned" | "payment";

const auth = { auth: true } as const;

export const campground = {
  list: () => request<CampSummary[]>("/campgrounds", auth),
  detail: (id: string) => request<CampDetail>(`/campgrounds/${id}`, auth),
  setConfig: (id: string, body: CampConfig) =>
    request<CampDetail>(`/campgrounds/${id}/config`, {
      method: "PUT",
      body,
      ...auth,
    }),
  addSeason: (id: string, body: Omit<Season, "id">) =>
    request<CampDetail>(`/campgrounds/${id}/seasons`, {
      method: "POST",
      body,
      ...auth,
    }),
  deleteSeason: (id: string) =>
    request<{ deleted: boolean }>(`/campground-seasons/${id}`, {
      method: "DELETE",
      ...auth,
    }),
  availability: (id: string, from: string, to: string) =>
    request<{ site: Site; stays: Stay[]; free: boolean }[]>(
      `/campgrounds/${id}/availability?from=${from}&to=${to}`,
      auth
    ),
  quote: (id: string, body: QuoteInput) =>
    request<Quote>(`/campgrounds/${id}/quote`, {
      method: "POST",
      body,
      ...auth,
    }),
  book: (id: string, body: StayInput) =>
    request<Stay>(`/campgrounds/${id}/stays`, {
      method: "POST",
      body,
      ...auth,
    }),
  board: (id: string, date: string) =>
    request<Board>(`/campgrounds/${id}/board?date=${date}`, auth),
  act: (
    stayId: string,
    action: StayActionName,
    extra: { amount_cents?: number; note?: string } = {}
  ) =>
    request<Stay>(`/stays/${stayId}`, {
      method: "PATCH",
      body: { action, ...extra },
      ...auth,
    }),
};

export interface PublicCamp {
  map_id: string;
  name: string;
  company: string;
  check_in_time: string;
  check_out_time: string;
  max_nights: number;
  deposit_pct: number;
  addons: Addon[];
  policies: string | null;
  sites: { site: Site; free: boolean | null }[];
}

export interface GuestStay {
  stay: Stay;
  campground: string;
  check_in_time: string;
  check_out_time: string;
  policies: string | null;
  can_cancel: boolean;
}

export const publicCamp = {
  view: (id: string, tenant = DEFAULT_TENANT, from?: string, to?: string) =>
    request<PublicCamp>(
      `/public/campgrounds/${id}${from && to ? `?from=${from}&to=${to}` : ""}`,
      { tenant }
    ),
  quote: (id: string, body: QuoteInput, tenant = DEFAULT_TENANT) =>
    request<Quote>(`/public/campgrounds/${id}/quote`, {
      method: "POST",
      body,
      tenant,
    }),
  request: (id: string, body: StayInput, tenant = DEFAULT_TENANT) =>
    request<{ stay: Stay; link: string }>(`/public/campgrounds/${id}/stays`, {
      method: "POST",
      body,
      tenant,
    }),
  guest: (token: string) => request<GuestStay>(`/public/stays/${token}`),
  cancel: (token: string) =>
    request<GuestStay>(`/public/stays/${token}/cancel`, { method: "POST" }),
};

export const STATUS_WORDS: Record<StayStatus, string> = {
  held: "Request",
  confirmed: "Booked",
  checked_in: "In house",
  checked_out: "Checked out",
  cancelled: "Cancelled",
};

export function statusTone(
  s: StayStatus
): "good" | "warn" | "info" | "neutral" | "bad" {
  return s === "held"
    ? "warn"
    : s === "confirmed"
      ? "info"
      : s === "checked_in"
        ? "good"
        : s === "cancelled"
          ? "bad"
          : "neutral";
}

/** ISO date `n` days from `from` (YYYY-MM-DD, local). */
export function addDays(from: string, n: number): string {
  const d = new Date(`${from}T12:00:00`);
  d.setDate(d.getDate() + n);
  return d.toISOString().slice(0, 10);
}

export function todayIso(): string {
  const d = new Date();
  d.setMinutes(d.getMinutes() - d.getTimezoneOffset());
  return d.toISOString().slice(0, 10);
}

/** Which day columns a stay covers in a window starting `from`, `days` long. */
export function span(
  stay: Pick<Stay, "check_in" | "check_out">,
  from: string,
  days: number
): [number, number] | null {
  const day = (s: string) =>
    Math.round(
      (new Date(`${s}T12:00:00`).getTime() -
        new Date(`${from}T12:00:00`).getTime()) /
        86_400_000
    );
  const a = Math.max(0, day(stay.check_in));
  const b = Math.min(days, day(stay.check_out));
  return b > a ? [a, b] : null;
}
