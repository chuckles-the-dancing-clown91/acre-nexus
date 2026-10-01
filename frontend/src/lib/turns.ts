// Turnovers and other step processes (roadmap areas 3 and 7).

import { request } from "@/lib/api";

export interface Step {
  id: string;
  position: number;
  key: string;
  title: string;
  description: string | null;
  owner_role: string;
  assignee_user_id: string | null;
  depends_on: string[];
  /** Titles of the unfinished steps this one waits on. */
  waiting_on: string[];
  due_on: string | null;
  overdue: boolean;
  required: boolean;
  requires_photo: boolean;
  status: "blocked" | "ready" | "doing" | "done" | "skipped";
  done_at: string | null;
  skip_reason: string | null;
  note: string | null;
  ticket_id: string | null;
  ticket_status: string | null;
  cost_cents: number | null;
  cost_label: string | null;
}

export interface Turn {
  id: string;
  kind: string;
  property_id: string;
  property_name: string;
  unit_id: string | null;
  unit_number: string | null;
  title: string;
  status: "active" | "done" | "cancelled";
  started_on: string;
  target_date: string | null;
  finished_on: string | null;
  override_reason: string | null;
  done: number;
  total: number;
  unmet_required: string[];
  days_open: number;
  overdue: boolean;
  cost_cents: number;
  cost_label: string;
  steps: Step[] | null;
}

export interface TemplateStep {
  key: string;
  title: string;
  description: string | null;
  owner_role: string;
  depends_on: string[];
  due_offset_days: number;
  required: boolean;
  requires_photo: boolean;
  ticket_category: string | null;
  ticket_priority: string | null;
}

export interface Template {
  id: string;
  kind: string;
  name: string;
  is_default: boolean;
  active: boolean;
  steps: TemplateStep[];
}

export const OWNER_ROLES = [
  "office",
  "maintenance",
  "vendor",
  "leasing",
  "owner",
] as const;

export type StepAction =
  "start" | "complete" | "skip" | "reopen" | "assign" | "note";

const post = <T>(path: string, body: unknown = {}) =>
  request<T>(path, { method: "POST", auth: true, body });

export const turns = {
  list: (
    f: { status?: string; property_id?: string; unit_id?: string } = {}
  ) => {
    const p = new URLSearchParams({ kind: "turnover" });
    for (const [k, v] of Object.entries(f)) if (v) p.set(k, v);
    return request<Turn[]>(`/processes?${p}`, { auth: true });
  },
  get: (id: string) => request<Turn>(`/processes/${id}`, { auth: true }),
  start: (
    unitId: string,
    body: { template_id?: string; started_on?: string }
  ) => post<Turn>(`/units/${unitId}/turn`, body),
  step: (
    id: string,
    body: {
      action: StepAction;
      note?: string;
      reason?: string;
      cost_cents?: number;
    }
  ) => post<Turn>(`/process-steps/${id}/action`, body),
  stepTicket: (id: string, body: { title?: string; priority?: string } = {}) =>
    post<Turn>(`/process-steps/${id}/ticket`, body),
  finish: (id: string, override_reason?: string) =>
    post<Turn>(`/processes/${id}/finish`, { override_reason }),
  cancel: (id: string) => post<Turn>(`/processes/${id}/cancel`),
  templates: () =>
    request<Template[]>("/process-templates?kind=turnover", { auth: true }),
  createTemplate: (body: Omit<Template, "id" | "active" | "kind">) =>
    post<Template>("/process-templates", body),
  saveTemplate: (
    id: string,
    body: { name: string; is_default: boolean; steps: TemplateStep[] }
  ) =>
    request<Template>(`/process-templates/${id}`, {
      method: "PUT",
      auth: true,
      body,
    }),
};

export function stepTone(
  s: Step["status"]
): "good" | "info" | "warn" | "neutral" {
  if (s === "done") return "good";
  if (s === "doing") return "info";
  if (s === "ready") return "warn";
  return "neutral";
}
