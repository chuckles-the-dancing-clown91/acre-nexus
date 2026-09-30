// Typed client for the back office (Vantedge phase 2B): the team and the
// clock, timesheets, shifts and time off, expenses and mileage, work-order
// costing and billing owners, payroll / profit / tax reports, Gusto, and the
// owner CRM. Mirrors the Rust DTOs in backend/crates/api/src/routes/{team,
// backoffice,crm}. Money is integer cents; durations are minutes.

import { api, request } from "./api";

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/** `$1,234.56` from cents. */
export function money(cents: number | null | undefined): string {
  const c = cents ?? 0;
  const sign = c < 0 ? "-" : "";
  return `${sign}$${(Math.abs(c) / 100).toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

/** `13:05` from minutes. */
export function hm(minutes: number | null | undefined): string {
  const m = Math.max(0, Math.round(minutes ?? 0));
  return `${Math.floor(m / 60)}:${String(m % 60).padStart(2, "0")}`;
}

/** `45.1%` from basis points. */
export function pct(bps: number | null | undefined): string {
  return `${((bps ?? 0) / 100).toFixed(1)}%`;
}

/** Dollars typed by a person ("12.50", "$1,200") → cents, or null. */
export function toCents(input: string): number | null {
  const n = Number(input.replace(/[$,\s]/g, ""));
  return Number.isFinite(n) && input.trim() !== "" ? Math.round(n * 100) : null;
}

/** A local `YYYY-MM-DD` for a Date. */
export function isoDate(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** The Monday of the week containing `d`. */
export function weekStart(d: Date): Date {
  const x = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  const day = (x.getDay() + 6) % 7;
  x.setDate(x.getDate() - day);
  return x;
}

/** A local datetime-input value (`YYYY-MM-DDTHH:mm`) → RFC 3339 with offset. */
export function localToRfc3339(v: string): string {
  const d = new Date(v);
  const off = -d.getTimezoneOffset();
  const sign = off >= 0 ? "+" : "-";
  const p = (n: number) => String(Math.floor(Math.abs(n))).padStart(2, "0");
  return `${v.length === 16 ? `${v}:00` : v}${sign}${p(off / 60)}:${p(off % 60)}`;
}

/** An RFC 3339 instant → a datetime-input value in local time. */
export function rfc3339ToLocal(s: string): string {
  const d = new Date(s);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}T${p(d.getHours())}:${p(d.getMinutes())}`;
}

/** Download an authenticated export (CSV/PDF) under `filename`. */
export async function download(path: string, filename: string) {
  const blob = await api.downloadReport(path);
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Open an authenticated PDF in a new tab (to print). */
export async function openPdf(path: string) {
  const blob = await api.downloadReport(path);
  const url = URL.createObjectURL(blob);
  window.open(url, "_blank", "noopener");
  setTimeout(() => URL.revokeObjectURL(url), 60_000);
}

const q = (
  params: Record<string, string | number | boolean | undefined | null>
) => {
  const s = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null && v !== "") s.set(k, String(v));
  }
  const out = s.toString();
  return out ? `?${out}` : "";
};

// ---------------------------------------------------------------------------
// Team & time
// ---------------------------------------------------------------------------

export type EntryKind =
  | "work_order"
  | "project"
  | "property"
  | "travel"
  | "shop"
  | "admin"
  | "other";

export const ENTRY_KIND_LABELS: Record<EntryKind, string> = {
  work_order: "Work order",
  project: "Rehab project",
  property: "Property",
  travel: "Travel",
  shop: "Shop / equipment",
  admin: "Office / admin",
  other: "Other",
};

export type EmploymentType =
  | "full_time"
  | "part_time"
  | "seasonal"
  | "contractor";

export const EMPLOYMENT_LABELS: Record<EmploymentType, string> = {
  full_time: "Full-time",
  part_time: "Part-time",
  seasonal: "Seasonal",
  contractor: "Contractor (1099)",
};

export interface EmployeeProfile {
  title: string | null;
  employment_type: EmploymentType;
  /** Only with payroll:read. */
  pay_rate_cents: number | null;
  bill_rate_cents: number | null;
  hire_date: string | null;
  end_date: string | null;
  current: boolean;
  weekly_hours_target: number;
  default_vehicle: "company" | "personal";
  mileage_reimbursed: boolean;
  overtime_eligible: boolean;
  emergency_contact_name: string | null;
  emergency_contact_phone: string | null;
  calendar_color: string;
  notes: string | null;
}

export interface Employee {
  user_id: string;
  name: string;
  email: string;
  personas: string[];
  profile: EmployeeProfile | null;
  clocked_in: boolean;
  week_minutes: number;
}

export type ProfileInput = Partial<
  Omit<EmployeeProfile, "current" | "overtime_eligible">
>;

export interface TimeEntry {
  id: string;
  user_id: string;
  user_name: string;
  kind: EntryKind;
  maintenance_ticket_id: string | null;
  work_order_title: string | null;
  rehab_project_id: string | null;
  project_name: string | null;
  property_id: string | null;
  property_name: string | null;
  started_at: string;
  ended_at: string | null;
  break_minutes: number;
  minutes: number;
  notes: string | null;
  pay_rate_cents: number | null;
  bill_rate_cents: number | null;
  labor_cost_cents: number | null;
  approved: boolean;
  approved_at: string | null;
  missed_punch: boolean;
  missed_punch_reason: string | null;
  needs_review: boolean;
  claimed_end: string | null;
  punch_note: string | null;
  in_distance_m: number | null;
  out_distance_m: number | null;
  away: boolean;
  billed: boolean;
}

export interface Target {
  kind: EntryKind;
  maintenance_ticket_id?: string | null;
  rehab_project_id?: string | null;
  property_id?: string | null;
}

export interface EntryInput extends Target {
  started_at: string;
  ended_at?: string | null;
  break_minutes?: number;
  notes?: string | null;
  /** Office only. */
  user_id?: string;
}

export interface Location {
  lat: number;
  lng: number;
  accuracy_m?: number;
}

export interface ClockState {
  open: TimeEntry | null;
  today_minutes: number;
  week_minutes: number;
  missed_punches: number;
  records_location: boolean;
  overtime_rule: string;
}

export interface WeekHours {
  week_of: string;
  days_worked: number;
  minutes: number;
  regular_minutes: number;
  overtime_minutes: number;
  double_minutes: number;
  unapproved_minutes: number;
  gross_cents: number;
}

export interface WorkOption {
  kind: "work_order" | "project" | "property";
  id: string;
  label: string;
  property_name: string | null;
  mine: boolean;
}

export interface PersonHours {
  user_id: string;
  name: string;
  minutes: number;
  on_work_minutes: number;
  other_minutes: number;
  unapproved_minutes: number;
  missed_punches: number;
  clocked_in: boolean;
  labor_cost_cents: number | null;
  work_orders: number;
}

export interface Shift {
  id: string;
  user_id: string;
  user_name: string;
  starts_at: string;
  ends_at: string;
  kind: "work" | "on_call" | "training";
  property_id: string | null;
  notes: string | null;
  minutes: number;
}

export interface ShiftInput {
  user_id: string;
  starts_at: string;
  ends_at: string;
  kind?: Shift["kind"];
  property_id?: string | null;
  notes?: string | null;
}

export interface TimeOff {
  id: string;
  user_id: string;
  user_name: string;
  starts_on: string;
  ends_on: string;
  days: number;
  kind: "vacation" | "sick" | "personal" | "unpaid";
  status: "pending" | "approved" | "denied" | "cancelled";
  reason: string | null;
  reviewed_by: string | null;
  review_note: string | null;
  created_at: string;
}

export const team = {
  roster: () => request<Employee[]>("/team", { auth: true }),
  saveProfile: (userId: string, body: ProfileInput) =>
    request<EmployeeProfile>(`/team/${userId}`, {
      method: "PUT",
      auth: true,
      body,
    }),

  time: (
    p: { from?: string; to?: string; user_id?: string; status?: string } = {}
  ) => request<TimeEntry[]>(`/team/time${q(p)}`, { auth: true }),
  addTime: (body: EntryInput) =>
    request<TimeEntry>("/team/time", { method: "POST", auth: true, body }),
  editTime: (id: string, body: EntryInput) =>
    request<TimeEntry>(`/team/time/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  deleteTime: (id: string) =>
    request<{ ok: boolean }>(`/team/time/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  approve: (id: string) =>
    request<TimeEntry>(`/team/time/${id}/approve`, {
      method: "POST",
      auth: true,
    }),
  approveMany: (ids: string[]) =>
    request<{ approved: number; skipped: { id: string; reason: string }[] }>(
      "/team/time/approve",
      { method: "POST", auth: true, body: { ids } }
    ),
  resolve: (id: string, body: { ended_at?: string; break_minutes?: number }) =>
    request<TimeEntry>(`/team/time/${id}/resolve`, {
      method: "POST",
      auth: true,
      body,
    }),
  clockOut: (id: string) =>
    request<TimeEntry>(`/team/time/${id}/clock-out`, {
      method: "POST",
      auth: true,
    }),
  summary: (p: { from?: string; to?: string } = {}) =>
    request<PersonHours[]>(`/team/time/summary${q(p)}`, { auth: true }),

  shifts: (p: { from?: string; to?: string; user_id?: string } = {}) =>
    request<Shift[]>(`/team/shifts${q(p)}`, { auth: true }),
  addShift: (body: ShiftInput) =>
    request<Shift>("/team/shifts", { method: "POST", auth: true, body }),
  editShift: (id: string, body: ShiftInput) =>
    request<Shift>(`/team/shifts/${id}`, { method: "PATCH", auth: true, body }),
  deleteShift: (id: string) =>
    request<{ ok: boolean }>(`/team/shifts/${id}`, {
      method: "DELETE",
      auth: true,
    }),

  timeOff: (status?: string) =>
    request<TimeOff[]>(`/team/time-off${q({ status })}`, { auth: true }),
  reviewTimeOff: (id: string, approve: boolean, note?: string) =>
    request<TimeOff>(`/team/time-off/${id}/review`, {
      method: "POST",
      auth: true,
      body: { approve, note },
    }),
};

/** Self-service: anyone with an employee profile. */
export const me = {
  clock: () => request<ClockState>("/me/clock", { auth: true }),
  clockIn: (body: Target & { notes?: string; location?: Location }) =>
    request<ClockState>("/me/clock/in", { method: "POST", auth: true, body }),
  clockOut: (
    body: { location?: Location; break_minutes?: number; notes?: string } = {}
  ) =>
    request<ClockState>("/me/clock/out", { method: "POST", auth: true, body }),
  time: (p: { from?: string; to?: string } = {}) =>
    request<TimeEntry[]>(`/me/time${q(p)}`, { auth: true }),
  addTime: (body: EntryInput) =>
    request<TimeEntry>("/me/time", { method: "POST", auth: true, body }),
  editTime: (id: string, body: EntryInput) =>
    request<TimeEntry>(`/me/time/${id}`, { method: "PATCH", auth: true, body }),
  deleteTime: (id: string) =>
    request<{ ok: boolean }>(`/me/time/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  claim: (id: string, ended_at: string, note?: string) =>
    request<TimeEntry>(`/me/time/${id}/claim`, {
      method: "POST",
      auth: true,
      body: { ended_at, note },
    }),
  hours: (p: { from?: string; to?: string } = {}) =>
    request<WeekHours[]>(`/me/hours${q(p)}`, { auth: true }),
  work: () => request<WorkOption[]>("/me/work", { auth: true }),
  shifts: (p: { from?: string; to?: string } = {}) =>
    request<Shift[]>(`/me/shifts${q(p)}`, { auth: true }),
  timeOff: () => request<TimeOff[]>("/me/time-off", { auth: true }),
  requestTimeOff: (body: {
    starts_on: string;
    ends_on: string;
    kind?: TimeOff["kind"];
    reason?: string;
  }) => request<TimeOff>("/me/time-off", { method: "POST", auth: true, body }),
  cancelTimeOff: (id: string) =>
    request<TimeOff>(`/me/time-off/${id}`, { method: "DELETE", auth: true }),
  expenses: (p: { from?: string; to?: string } = {}) =>
    request<Expense[]>(`/me/expenses${q(p)}`, { auth: true }),
  addExpense: (body: ExpenseInput) =>
    request<Expense>("/me/expenses", { method: "POST", auth: true, body }),
  editExpense: (id: string, body: ExpenseInput) =>
    request<Expense>(`/me/expenses/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  deleteExpense: (id: string) =>
    request<{ ok: boolean }>(`/me/expenses/${id}`, {
      method: "DELETE",
      auth: true,
    }),
};

/** Ask the browser where we are (resolves null when refused or unavailable). */
export function currentLocation(): Promise<Location | null> {
  return new Promise((resolve) => {
    if (typeof navigator === "undefined" || !navigator.geolocation) {
      resolve(null);
      return;
    }
    navigator.geolocation.getCurrentPosition(
      (p) =>
        resolve({
          lat: p.coords.latitude,
          lng: p.coords.longitude,
          accuracy_m: p.coords.accuracy,
        }),
      () => resolve(null),
      { enableHighAccuracy: true, timeout: 8000, maximumAge: 60_000 }
    );
  });
}

// ---------------------------------------------------------------------------
// Expenses & mileage
// ---------------------------------------------------------------------------

export const EXPENSE_CATEGORIES = [
  "materials",
  "mileage",
  "fuel",
  "equipment",
  "repairs",
  "vehicle",
  "insurance",
  "payroll",
  "marketing",
  "software",
  "licenses",
  "other",
] as const;
export type ExpenseCategory = (typeof EXPENSE_CATEGORIES)[number];

export interface Expense {
  id: string;
  incurred_on: string;
  category: ExpenseCategory;
  vendor: string | null;
  description: string;
  amount_cents: number;
  miles: number | null;
  mileage_rate: number | null;
  tax_deductible: boolean;
  vehicle: "company" | "personal" | "none";
  reimbursable: boolean;
  reimbursed: boolean;
  billable_to_owner: boolean;
  billed: boolean;
  user_id: string | null;
  user_name: string | null;
  maintenance_ticket_id: string | null;
  work_order_title: string | null;
  rehab_project_id: string | null;
  project_name: string | null;
  property_id: string | null;
  property_name: string | null;
  asset_id: string | null;
  details: Record<string, unknown>;
  receipts: number;
  created_at: string;
}

export interface ExpenseInput {
  incurred_on: string;
  category: ExpenseCategory;
  vendor?: string | null;
  description?: string;
  amount_cents?: number | null;
  miles?: number | null;
  tax_deductible?: boolean;
  vehicle?: Expense["vehicle"];
  reimbursable?: boolean;
  billable_to_owner?: boolean;
  maintenance_ticket_id?: string | null;
  rehab_project_id?: string | null;
  property_id?: string | null;
  asset_id?: string | null;
  details?: Record<string, unknown>;
  /** Office only. */
  user_id?: string | null;
}

export interface Receipt {
  document_id: string;
  filename: string;
  label: string | null;
  mime_type: string;
  download_url: string;
  created_at: string;
}

export const expenses = {
  list: (
    p: {
      from?: string;
      to?: string;
      category?: string;
      user_id?: string;
      work_order_id?: string;
      project_id?: string;
      property_id?: string;
    } = {}
  ) => request<Expense[]>(`/expenses${q(p)}`, { auth: true }),
  create: (body: ExpenseInput) =>
    request<Expense>("/expenses", { method: "POST", auth: true, body }),
  update: (id: string, body: ExpenseInput) =>
    request<Expense>(`/expenses/${id}`, { method: "PATCH", auth: true, body }),
  remove: (id: string) =>
    request<{ ok: boolean }>(`/expenses/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  reimburse: (ids: string[]) =>
    request<{ reimbursed: number; total_cents: number }>(
      "/expenses/reimburse",
      {
        method: "POST",
        auth: true,
        body: { ids },
      }
    ),
  receipts: (id: string) =>
    request<Receipt[]>(`/expenses/${id}/receipts`, { auth: true }),
  /** Register a receipt, then PUT the file's bytes to the returned URL. */
  uploadReceipt: async (id: string, file: File, label = "receipt") => {
    const reg = await request<{ document_id: string; upload_url: string }>(
      `/expenses/${id}/receipts`,
      {
        method: "POST",
        auth: true,
        body: {
          filename: file.name,
          mime_type: file.type || "application/octet-stream",
          size_bytes: file.size,
          label,
        },
      }
    );
    const res = await fetch(reg.upload_url, {
      method: "PUT",
      body: file,
      headers: { "Content-Type": file.type || "application/octet-stream" },
    });
    if (!res.ok) throw new Error("the upload didn't go through — try again");
    return reg.document_id;
  },
};

// ---------------------------------------------------------------------------
// Costing & billing owners
// ---------------------------------------------------------------------------

export type WorkKind = "work-orders" | "rehab-projects";

export interface Cost {
  minutes: number;
  labor_pay_cents: number;
  overtime_premium_cents: number;
  burden_cents: number;
  labor_cents: number;
  parts_cents: number;
  other_lines_cents: number;
  mileage_cents: number;
  expenses_cents: number;
  costs_cents: number;
  overhead_cents: number;
  billed_cents: number;
  unbilled_cents: number;
  revenue_cents: number;
  gross_cents: number;
  gross_bps: number;
  net_cents: number;
  vendor_bills_cents: number;
  owner_total_cents: number;
  bill_rate_for_target_cents: number;
}

export interface Costs extends Cost {
  title: string;
  target_margin_bps: number;
  under_target: boolean;
  overtime_rule: string;
  by_person: {
    user_id: string;
    name: string;
    minutes: number;
    billable_cents: number;
  }[];
}

export interface BillLine {
  description: string;
  amount_cents: number;
}

export interface BillPreview {
  title: string;
  property_id: string;
  lines: BillLine[];
  total_cents: number;
  markup_bps: number;
  held_back: string[];
  previous_bills: {
    id: string;
    bill_number: string;
    status: string;
    amount_cents: number;
  }[];
}

export const costing = {
  costs: (kind: WorkKind, id: string) =>
    request<Costs>(`/costs/${kind}/${id}`, { auth: true }),
  preview: (kind: WorkKind, id: string) =>
    request<BillPreview>(`/costs/${kind}/${id}/bill-preview`, { auth: true }),
  billOwner: (
    kind: WorkKind,
    id: string,
    body: { memo?: string; due_date?: string } = {}
  ) =>
    request<{
      bill_id: string;
      bill_number: string;
      amount_cents: number;
      status: string;
      lines: BillLine[];
      held_back: string[];
    }>(`/costs/${kind}/${id}/bill-owner`, { method: "POST", auth: true, body }),
  sheetPath: (kind: WorkKind, id: string) => `/costs/${kind}/${id}/sheet.pdf`,
};

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

export interface PayrollRow {
  user_id: string;
  name: string;
  week_of: string;
  days_worked: number;
  entries: number;
  minutes: number;
  regular_minutes: number;
  overtime_minutes: number;
  double_minutes: number;
  rate_cents: number;
  gross_cents: number;
  premium_cents: number;
  mileage_paid_back_cents: number;
}

export interface PayrollReport {
  from: string;
  to: string;
  overtime_rule: string;
  approved_only: boolean;
  rows: PayrollRow[];
  total_minutes: number;
  total_gross_cents: number;
  total_mileage_paid_back_cents: number;
  excluded: string[];
}

export interface Rollup {
  key: string;
  label: string;
  jobs: number;
  minutes: number;
  revenue_cents: number;
  costs_cents: number;
  gross_cents: number;
  gross_bps: number;
  revenue_per_hour_cents: number;
}

export interface WorkRow extends Cost {
  kind: "work_order" | "project";
  id: string;
  title: string;
  property: string;
  category: string;
  status: string;
  month: string;
}

export interface ProfitReport {
  from: string;
  to: string;
  target_margin_bps: number;
  work: WorkRow[];
  by_property: Rollup[];
  by_technician: Rollup[];
  by_category: Rollup[];
  by_month: Rollup[];
  revenue_cents: number;
  costs_cents: number;
  gross_cents: number;
  gross_bps: number;
  net_cents: number;
  minutes: number;
  revenue_per_hour_cents: number;
  unbilled_cents: number;
  vendor_bills_cents: number;
  under_target: number;
}

export interface TaxReport {
  from: string;
  to: string;
  label: string;
  deductible_expenses_cents: number;
  nondeductible_expenses_cents: number;
  mileage_cents: number;
  miles: number;
  reimbursed_cents: number;
  w2_gross_cents: number;
  contractor_gross_cents: number;
  billed_to_owners_cents: number;
  missing_receipts: number;
  by_category: [string, number][];
  pay_by_person: {
    user_id: string;
    name: string;
    form: "W-2" | "1099-NEC";
    employment: string;
    minutes: number;
    overtime_minutes: number;
    double_minutes: number;
    gross_cents: number;
    mileage_paid_back_cents: number;
    miles: number;
  }[];
  key_dates: { date: string; what: string }[];
}

export interface BackOfficeDashboard {
  clocked_in: string[];
  team_size: number;
  hours_this_week_minutes: number;
  unapproved_entries: number;
  missed_punches: number;
  pending_time_off: number;
  expenses_to_reimburse: number;
  expenses_to_reimburse_cents: number;
  unbilled_time_cents: number;
  work_orders_with_unbilled_time: number;
  labor_cost_this_week_cents: number | null;
  billed_to_owners_this_month_cents: number;
}

export const reports = {
  payroll: (p: { from?: string; to?: string; approved_only?: boolean } = {}) =>
    request<PayrollReport>(`/reports/payroll${q(p)}`, { auth: true }),
  profit: (p: { from?: string; to?: string } = {}) =>
    request<ProfitReport>(`/reports/profit${q(p)}`, { auth: true }),
  taxes: (p: { year?: number; quarter?: number } = {}) =>
    request<TaxReport>(`/reports/taxes${q(p)}`, { auth: true }),
  dashboard: () =>
    request<BackOfficeDashboard>("/backoffice/dashboard", { auth: true }),
  /** Export paths (use with `download` / `openPdf`). */
  payrollExport: (p: {
    from?: string;
    to?: string;
    approved_only?: boolean;
    format: "csv" | "pdf";
  }) => `/reports/payroll/export${q(p)}`,
  profitExport: (p: {
    from?: string;
    to?: string;
    format: "csv" | "pdf";
    section?: string;
  }) => `/reports/profit/export${q(p)}`,
  taxesExport: (p: {
    year?: number;
    quarter?: number;
    format: "csv" | "pdf";
    section?: string;
  }) => `/reports/taxes/export${q(p)}`,
  timesheetsExport: (p: {
    from?: string;
    to?: string;
    user_id?: string;
    format: "csv" | "pdf";
  }) => `/reports/timesheets/export${q(p)}`,
};

export interface GustoLine {
  email: string;
  name: string;
  regular_hours: string;
  overtime_hours: string;
  double_overtime_hours: string;
}

export const gusto = {
  status: () =>
    request<{ company_uuid: string | null; live: boolean }>(
      "/payroll/gusto/status",
      {
        auth: true,
      }
    ),
  hours: (from: string, to: string) =>
    request<{
      from: string;
      to: string;
      lines: GustoLine[];
      left_out: string[];
    }>(`/payroll/gusto/hours${q({ from, to })}`, { auth: true }),
  hoursCsvPath: (from: string, to: string) =>
    `/payroll/gusto/hours.csv${q({ from, to })}`,
  push: (payroll_id: string, from: string, to: string) =>
    request<{
      pushed: string[];
      unmatched: string[];
      simulated: boolean;
      left_out: string[];
    }>("/payroll/gusto/push", {
      method: "POST",
      auth: true,
      body: { payroll_id, from, to },
    }),
};

// ---------------------------------------------------------------------------
// CRM
// ---------------------------------------------------------------------------

export type SubjectType = "owner" | "owner_lead" | "counterparty" | "property";
export type NoteKind =
  | "note"
  | "call"
  | "email"
  | "meeting"
  | "issue"
  | "text"
  | "update";

export interface CrmNote {
  id: string;
  subject_type: SubjectType;
  subject_id: string;
  subject_name: string | null;
  property_id: string | null;
  kind: NoteKind;
  body: string;
  pinned: boolean;
  follow_up_on: string | null;
  follow_up_done: boolean;
  follow_up_due: boolean;
  author: string | null;
  created_at: string;
}

export interface OwnerRow {
  id: string;
  kind: string;
  name: string;
  email: string | null;
  phone: string | null;
  notes: string | null;
  entities: string[];
  properties: number;
  doors: number;
  last_contact_at: string | null;
  open_follow_ups: number;
  follow_ups_due: number;
}

export type LeadStatus = "new" | "contacted" | "proposal" | "won" | "lost";
export const LEAD_STATUSES: LeadStatus[] = [
  "new",
  "contacted",
  "proposal",
  "won",
  "lost",
];
export const LEAD_SOURCES = [
  "website",
  "referral",
  "phone",
  "email",
  "event",
  "mailer",
  "other",
] as const;

export interface OwnerLead {
  id: string;
  name: string;
  company: string | null;
  email: string | null;
  phone: string | null;
  address: string | null;
  properties_count: number;
  doors: number;
  source: (typeof LEAD_SOURCES)[number];
  status: LeadStatus;
  lost_reason: string | null;
  monthly_rent_cents: number;
  fee_bps: number;
  monthly_fee_cents: number;
  notes: string | null;
  assigned_to: string | null;
  assigned_name: string | null;
  owner_id: string | null;
  next_follow_up: string | null;
  created_at: string;
}

export type OwnerLeadInput = Partial<
  Pick<
    OwnerLead,
    | "name"
    | "company"
    | "email"
    | "phone"
    | "address"
    | "properties_count"
    | "doors"
    | "source"
    | "status"
    | "lost_reason"
    | "monthly_rent_cents"
    | "notes"
    | "assigned_to"
  > & { fee_bps: number }
>;

export interface PipelineSummary {
  stages: {
    status: LeadStatus;
    leads: number;
    doors: number;
    monthly_fee_cents: number;
  }[];
  by_source: { source: string; leads: number; won: number; win_bps: number }[];
  weighted_monthly_fee_cents: number;
  win_bps: number;
  follow_ups_due: number;
}

export const crm = {
  notes: (subject_type: SubjectType, subject_id: string) =>
    request<CrmNote[]>(`/crm/notes${q({ subject_type, subject_id })}`, {
      auth: true,
    }),
  addNote: (body: {
    subject_type: SubjectType;
    subject_id: string;
    kind?: NoteKind;
    body: string;
    pinned?: boolean;
    follow_up_on?: string | null;
    property_id?: string | null;
  }) => request<CrmNote>("/crm/notes", { method: "POST", auth: true, body }),
  updateNote: (
    id: string,
    body: {
      body?: string;
      pinned?: boolean;
      follow_up_on?: string;
      follow_up_done?: boolean;
    }
  ) =>
    request<CrmNote>(`/crm/notes/${id}`, { method: "PATCH", auth: true, body }),
  deleteNote: (id: string) =>
    request<{ ok: boolean }>(`/crm/notes/${id}`, {
      method: "DELETE",
      auth: true,
    }),
  followUps: (all = false) =>
    request<CrmNote[]>(`/crm/follow-ups${q({ all: all || undefined })}`, {
      auth: true,
    }),
  owners: () => request<OwnerRow[]>("/crm/owners", { auth: true }),
  createOwner: (body: {
    name: string;
    kind?: string;
    email?: string;
    phone?: string;
    notes?: string;
  }) =>
    request<{ id: string; name: string }>("/crm/owners", {
      method: "POST",
      auth: true,
      body,
    }),
  updateOwner: (
    id: string,
    body: {
      name: string;
      kind?: string;
      email?: string;
      phone?: string;
      notes?: string;
    }
  ) =>
    request<{ id: string; name: string }>(`/crm/owners/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  leads: (status?: LeadStatus) =>
    request<OwnerLead[]>(`/crm/owner-leads${q({ status })}`, { auth: true }),
  createLead: (body: OwnerLeadInput) =>
    request<OwnerLead>("/crm/owner-leads", {
      method: "POST",
      auth: true,
      body,
    }),
  updateLead: (id: string, body: OwnerLeadInput) =>
    request<OwnerLead>(`/crm/owner-leads/${id}`, {
      method: "PATCH",
      auth: true,
      body,
    }),
  convertLead: (id: string) =>
    request<OwnerLead>(`/crm/owner-leads/${id}/convert`, {
      method: "POST",
      auth: true,
    }),
  pipeline: () =>
    request<PipelineSummary>("/crm/owner-leads/summary", { auth: true }),
  proposalPath: (id: string) => `/crm/owner-leads/${id}/proposal.pdf`,
};
