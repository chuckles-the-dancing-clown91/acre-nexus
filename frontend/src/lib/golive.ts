// The go-live checklist: provider readiness, platform checks, backups.

import { request } from "@/lib/api";

export type Readiness =
  "ready" | "untested" | "failing" | "missing" | "simulated";

export interface ProviderCheck {
  key: string;
  label: string;
  what: string;
  href: string;
  live: boolean;
  requirements: { label: string; present: boolean }[];
  last_call_at: string | null;
  last_call_ok: boolean | null;
  last_error: string | null;
  failures_7d: number;
  webhook_expected: boolean;
  last_webhook_at: string | null;
  readiness: Readiness;
}

export interface BackupRun {
  started_at: string;
  finished_at: string | null;
  ok: boolean;
  bytes: number | null;
  location: string | null;
  detail: string | null;
}

export interface GoLive {
  providers: ProviderCheck[];
  platform: { key: string; label: string; ok: boolean; detail: string }[];
  last_backup: BackupRun | null;
  last_good_backup_at: string | null;
  last_drill: BackupRun | null;
  live_providers: string;
}

export const goLive = {
  get: () => request<GoLive>("/go-live", { auth: true }),
};

export const READINESS: Record<
  Readiness,
  { word: string; tone: "good" | "warn" | "bad" | "neutral" }
> = {
  ready: { word: "Ready", tone: "good" },
  untested: { word: "Live, not used yet", tone: "warn" },
  failing: { word: "Failing", tone: "bad" },
  missing: { word: "Missing setup", tone: "bad" },
  simulated: { word: "Simulated", tone: "neutral" },
};

/** Sort: problems first, then live, then simulated. */
export function byUrgency(a: ProviderCheck, b: ProviderCheck): number {
  const rank: Record<Readiness, number> = {
    failing: 0,
    missing: 1,
    untested: 2,
    ready: 3,
    simulated: 4,
  };
  return (
    rank[a.readiness] - rank[b.readiness] || a.label.localeCompare(b.label)
  );
}

export function bytes(n: number | null): string {
  if (n === null) return "";
  if (n < 1024 * 1024) return `${Math.round(n / 1024)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
