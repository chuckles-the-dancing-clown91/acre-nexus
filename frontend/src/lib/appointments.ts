// Appointments: a visit someone has to be there for. Staff offer windows,
// the other side picks one from the portal or a link, reminders go out.

import { request } from "@/lib/api";

export interface Window {
  start: string;
  end: string;
}

export interface Appointment {
  id: string;
  property_id: string;
  property_name: string | null;
  unit_id: string | null;
  /** repair | showing | inspection | other */
  kind: string;
  /** ticket | task | lead | tour | custom */
  subject_type: string;
  subject_id: string | null;
  title: string;
  /** proposed | confirmed | declined | cancelled | done | no_show */
  status: string;
  windows: Window[];
  windows_words: string[];
  starts_at: string | null;
  ends_at: string | null;
  when_words: string | null;
  with_name: string | null;
  with_email: string | null;
  with_phone: string | null;
  /** resident | prospect | vendor | owner */
  with_role: string;
  assignee_user_id: string | null;
  assignee_name: string | null;
  vendor_entity_id: string | null;
  vendor_name: string | null;
  note: string | null;
  access_notes: string | null;
  confirmed_by: string | null;
  confirmed_at: string | null;
  proposed_start: string | null;
  proposed_end: string | null;
  proposed_words: string | null;
  outcome_note: string | null;
  created_at: string;
}

export interface PublicAppointment {
  id: string;
  company: string;
  title: string;
  kind: string;
  status: string;
  property: string;
  with_name: string | null;
  windows: Window[];
  windows_words: string[];
  when_words: string | null;
  note: string | null;
  timezone: string;
}

export interface WindowInput {
  start: string;
  end?: string;
}

export interface OfferInput {
  ticket_id?: string;
  property_id?: string;
  unit_id?: string;
  kind?: string;
  title?: string;
  windows: WindowInput[];
  with_name?: string;
  with_email?: string;
  with_phone?: string;
  with_role?: string;
  assignee_user_id?: string;
  vendor_entity_id?: string;
  note?: string;
  access_notes?: string;
  lead_id?: string;
}

const post = <T>(path: string, body: unknown, auth = true) =>
  request<T>(path, { method: "POST", auth, body });

export const appointments = {
  list: (q: {
    from?: string;
    to?: string;
    status?: string;
    assignee?: string;
    property_id?: string;
    subject_type?: string;
    subject_id?: string;
  }) => {
    const qs = new URLSearchParams();
    for (const [k, v] of Object.entries(q)) if (v) qs.set(k, v);
    const s = qs.toString();
    return request<Appointment[]>(`/appointments${s ? `?${s}` : ""}`, {
      auth: true,
    });
  },
  get: (id: string) =>
    request<Appointment>(`/appointments/${id}`, { auth: true }),
  offer: (body: OfferInput) =>
    post<Appointment & { link: string }>("/appointments", body),
  update: (
    id: string,
    body: {
      confirm?: WindowInput;
      status?: "cancelled" | "done" | "no_show";
      assignee_user_id?: string;
      note?: string;
      outcome_note?: string;
    }
  ) =>
    request<Appointment>(`/appointments/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),

  // The signed-in resident.
  mine: () => request<Appointment[]>("/my/appointments", { auth: true }),
  pick: (id: string, window: number) =>
    post<Appointment>(`/my/appointments/${id}/pick`, { window }),
  decline: (id: string, body: { propose?: WindowInput; reason?: string }) =>
    post<Appointment>(`/my/appointments/${id}/decline`, body),

  // Anyone with the link.
  publicView: (token: string) =>
    request<PublicAppointment>(`/public/book/${token}`),
  publicPick: (token: string, window: number) =>
    post<PublicAppointment>(`/public/book/${token}`, { window }, false),
  publicDecline: (
    token: string,
    body: { propose?: WindowInput; reason?: string }
  ) => post<PublicAppointment>(`/public/book/${token}/decline`, body, false),
};

/** "proposed" → "Waiting on a pick", and so on. */
export function statusWords(s: string, role = "resident"): string {
  switch (s) {
    case "proposed":
      return `Waiting on the ${role}`;
    case "confirmed":
      return "Confirmed";
    case "declined":
      return "Needs a new time";
    case "cancelled":
      return "Cancelled";
    case "done":
      return "Done";
    case "no_show":
      return "Nobody home";
    default:
      return s;
  }
}

/** Date and time fields → `YYYY-MM-DDTHH:MM`, which the server reads in the
 * workspace's time zone; blank or junk → null. */
export function instantFrom(date: string, time: string): string | null {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date) || !/^\d{2}:\d{2}$/.test(time))
    return null;
  if (Number.isNaN(new Date(`${date}T${time}`).getTime())) return null;
  return `${date}T${time}`;
}

/** The dates of a week, Monday first, for the day holding `d`. */
export function weekOf(d: Date): Date[] {
  const start = new Date(d);
  start.setHours(0, 0, 0, 0);
  const dow = (start.getDay() + 6) % 7;
  start.setDate(start.getDate() - dow);
  return Array.from({ length: 7 }, (_, i) => {
    const x = new Date(start);
    x.setDate(start.getDate() + i);
    return x;
  });
}

/** "2026-10-07" for a local date. */
export function ymd(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** The local day an appointment sits on: its confirmed start, else the first
 * offered window. */
export function dayOf(a: Appointment): string | null {
  const iso = a.starts_at ?? a.windows[0]?.start;
  return iso ? ymd(new Date(iso)) : null;
}

/** "9:00 AM" */
export function clock(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}
