// Settings → Schedule: the background jobs, what they did, when they run next.

import { request } from "@/lib/api";

export interface Job {
  id: string;
  kind: string;
  label: string | null;
  status: string;
  run_at: string;
  updated_at: string;
  attempts: number;
  last_error: string | null;
  result: Record<string, unknown> | null;
}

export const jobs = {
  schedule: () => request<Job[]>("/admin/jobs/schedule", { auth: true }),
  list: (f: { kind?: string; status?: string; before?: string } = {}) => {
    const p = new URLSearchParams({ limit: "50" });
    for (const [k, v] of Object.entries(f)) if (v) p.set(k, v);
    return request<Job[]>(`/admin/jobs?${p}`, { auth: true });
  },
  runNow: (id: string) =>
    request<Job>(`/admin/jobs/${id}/run-now`, { method: "POST", auth: true }),
};
